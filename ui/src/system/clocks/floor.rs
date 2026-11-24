// SPDX-License-Identifier: GPL-2.0-or-later
use super::status::{Reason, Status};
use super::thermal::{Guard, SAMPLE_PERIOD};
use super::Port;
use std::cmp::Ordering;
use std::io;
use std::time::Instant;

pub(super) const CPU: &str = "devices/system/cpu/cpufreq/policy4";

struct Files {
    minimum: String,
    available: String,
    current: String,
}
impl Files {
    fn new(dir: &str, min: &str, available: &str, cur: &str) -> Self {
        Self {
            minimum: format!("{dir}/{min}"),
            available: format!("{dir}/{available}"),
            current: format!("{dir}/{cur}"),
        }
    }
}

pub(super) struct Floor {
    pub(super) name: &'static str,
    pub(super) saved: Option<u64>,
    files: Option<Files>,
    target: Option<u64>,
    hz_per_unit: u64,
    thermal: Guard,
    disabled: Option<Reason>,
    state: Reason,
    logged: bool,
    restore_failed: bool,
    next_write: Instant,
}
impl Floor {
    pub(super) fn cpu(port: &dyn Port, enabled: bool, now: Instant) -> Self {
        let files = Files::new(CPU, "scaling_min_freq", "scaling_available_frequencies", "scaling_cur_freq");
        let thermal = Guard::binding(port, "cpu", enabled.then_some("cpufreq-cpu4"), now);
        Self::new(port, "cpu", Some(files), 1_000, thermal, enabled, now)
    }

    pub(super) fn gpu(port: &dyn Port, enabled: bool, dir: Option<&str>, now: Instant) -> Self {
        let files = dir.map(|dir| Files::new(dir, "min_freq", "available_frequencies", "cur_freq"));
        // Linux devfreq and its cooling device use the same parent device name.
        let cooling =
            dir.filter(|_| enabled).and_then(|dir| dir.rsplit('/').next()).map(|name| format!("devfreq-{name}"));
        let thermal = Guard::binding(port, "gpu", cooling.as_deref(), now);
        Self::new(port, "gpu", files, 1, thermal, enabled, now)
    }

    fn new(
        port: &dyn Port,
        name: &'static str,
        files: Option<Files>,
        hz_per_unit: u64,
        thermal: Guard,
        enabled: bool,
        now: Instant,
    ) -> Self {
        let disabled = if !enabled {
            Some(Reason::Disabled)
        } else if !files.as_ref().is_some_and(|f| port.read(&f.minimum).is_ok() && port.read(&f.available).is_ok()) {
            eprintln!("oos-ui: clock_floor device={name} absent; disabled");
            Some(Reason::Absent)
        } else {
            thermal.unavailable()
        };
        Self {
            name,
            saved: None,
            files,
            target: None,
            hz_per_unit,
            thermal,
            disabled,
            state: Reason::Stopped,
            logged: false,
            restore_failed: false,
            next_write: now,
        }
    }

    fn files(&self) -> io::Result<&Files> {
        self.files.as_ref().ok_or_else(|| io::Error::other("clock device is absent"))
    }

    pub(super) fn inhibit(
        &mut self,
        port: &dyn Port,
        requested: bool,
        critical: Option<Reason>,
        now: Instant,
    ) -> Option<Reason> {
        self.disabled.or_else(|| {
            if requested {
                self.thermal.inhibit(port, self.name, now).max(critical)
            } else {
                Some(Reason::Stopped)
            }
        })
    }

    fn attempted(&self) -> bool {
        matches!(self.state, Reason::Applied | Reason::ApplyFailed)
    }

    pub(super) fn should_apply(&self, target: u64, now: Instant) -> bool {
        !self.attempted()
            || self.target != Some(target)
            || (self.state == Reason::ApplyFailed && now >= self.next_write)
    }

    pub(super) fn finish_apply(&mut self, target: u64, now: Instant, result: io::Result<()>) {
        self.state = match result {
            Ok(()) => Reason::Applied,
            Err(error) => {
                if self.state != Reason::ApplyFailed || self.target != Some(target) {
                    eprintln!("oos-ui: clock_floor device={} target={target} error={error}", self.name);
                }
                Reason::ApplyFailed
            }
        };
        self.target = Some(target);
        self.next_write = now + SAMPLE_PERIOD;
    }

    pub(super) fn restore_failed(&self) -> bool {
        self.restore_failed
    }

    pub(super) fn release(&mut self, port: &mut dyn Port, inhibit: Option<Reason>, now: Instant, force: bool) -> bool {
        let immediate = force || self.attempted();
        if let Some(reason) = inhibit {
            self.state = reason;
        }
        if self.saved.is_none() || (!immediate && now < self.next_write) {
            return false;
        }
        self.next_write = now + SAMPLE_PERIOD;
        self.restore(port).is_ok()
    }

    fn supports(available: &str, value: u64) -> bool {
        available.split_whitespace().any(|frequency| frequency.parse() == Ok(value))
    }

    pub(super) fn prepare(&self, io: &dyn Port, target: u64) -> io::Result<u64> {
        let files = self.files()?;
        let previous = match self.saved {
            Some(previous) => previous,
            None => read_freq(io, &files.minimum)?,
        };
        let applied = previous.max(target);
        let available = io.read(&files.available)?;
        if !Self::supports(&available, applied) || !Self::supports(&available, previous) {
            return Err(io::Error::other("minimum is not in available frequencies"));
        }
        Ok(previous)
    }

    pub(super) fn apply(&mut self, io: &mut dyn Port, previous: u64, target: u64) -> io::Result<()> {
        // The controller persisted this minimum before calling apply.
        self.saved = Some(previous);
        let applied = previous.max(target);
        write_minimum(io, &self.files()?.minimum, applied)?;
        if !self.logged {
            eprintln!("oos-ui: clock_floor device={} applied={} previous={}", self.name, applied, previous);
            self.logged = true;
        }
        Ok(())
    }

    pub(super) fn restore(&mut self, io: &mut dyn Port) -> io::Result<()> {
        let Some(previous) = self.saved else { return Ok(()) };
        let result = self.restore_value(io, previous);
        match &result {
            Err(error) if !self.restore_failed => {
                eprintln!("oos-ui: clock_floor device={} running=false error={error}", self.name);
            }
            Ok(()) if self.restore_failed => {
                eprintln!("oos-ui: clock_floor device={} restored={previous}", self.name);
            }
            _ => {}
        }
        self.restore_failed = result.is_err();
        result
    }

    fn restore_value(&mut self, io: &mut dyn Port, previous: u64) -> io::Result<()> {
        let files = self.files()?;
        if !Self::supports(&io.read(&files.available)?, previous) {
            return Err(io::Error::other("saved minimum is no longer available"));
        }
        write_minimum(io, &files.minimum, previous)?;
        self.saved = None;
        Ok(())
    }

    pub(super) fn status(&self, port: &dyn Port, target: Option<u64>, recovery_failed: bool) -> Status {
        let current_hz = self
            .files
            .as_ref()
            .and_then(|files| read_freq(port, &files.current).ok())
            .and_then(|value| value.checked_mul(self.hz_per_unit));
        let (applied, reason) = self.status_reason(port, target, recovery_failed);
        Status::new(self.name, target.unwrap_or_default() * self.hz_per_unit, current_hz, applied, reason)
    }

    fn status_reason(&self, port: &dyn Port, target: Option<u64>, recovery_failed: bool) -> (bool, Reason) {
        if recovery_failed {
            (false, Reason::RecoveryFailed)
        } else if self.restore_failed {
            let minimum = self.compare_minimum(port, target.or(self.target));
            (minimum.is_ok_and(|order| order != Ordering::Less), Reason::RestoreFailed)
        } else if self.attempted() {
            match self.compare_minimum(port, target) {
                Ok(Ordering::Equal) => (true, Reason::Applied),
                result if self.state == Reason::ApplyFailed => (result == Ok(Ordering::Greater), Reason::ApplyFailed),
                Ok(Ordering::Greater) => (true, Reason::Applied),
                Ok(Ordering::Less) => (false, Reason::MinimumChanged),
                Err(reason) => (false, reason),
            }
        } else {
            (false, self.disabled.unwrap_or(self.state))
        }
    }

    fn compare_minimum(&self, port: &dyn Port, target: Option<u64>) -> Result<Ordering, Reason> {
        let (Some(previous), Some(target)) = (self.saved, target) else { return Err(Reason::MinimumUnknown) };
        let minimum =
            self.files().and_then(|files| read_freq(port, &files.minimum)).map_err(|_| Reason::MinimumUnknown)?;
        Ok(minimum.cmp(&previous.max(target)))
    }
}

fn write_minimum(port: &mut dyn Port, path: &str, value: u64) -> io::Result<()> {
    let write = port.write(path, value);
    let minimum = read_freq(port, path);
    if minimum.as_ref().is_ok_and(|&minimum| minimum == value) {
        return Ok(());
    }
    write?;
    minimum?;
    Err(io::Error::other("minimum read-back mismatch"))
}

fn read_freq(port: &dyn Port, path: &str) -> io::Result<u64> {
    port.read(path)?.trim().parse().map_err(io::Error::other)
}
