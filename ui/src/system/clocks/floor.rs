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

#[derive(Clone, Copy)]
enum Phase {
    Idle,
    Prepared { previous: u64 },
    Raised { previous: u64, minimum: u64 },
    RestorePending { previous: u64, verified: Option<u64> },
}

impl Phase {
    fn previous(self) -> Option<u64> {
        match self {
            Self::Idle => None,
            Self::Prepared { previous } | Self::Raised { previous, .. } | Self::RestorePending { previous, .. } => {
                Some(previous)
            }
        }
    }

    fn verified(self) -> Option<u64> {
        match self {
            Self::Raised { minimum, .. } => Some(minimum),
            Self::RestorePending { verified, .. } => verified,
            Self::Idle | Self::Prepared { .. } => None,
        }
    }
}

pub(super) struct Floor {
    pub(super) name: &'static str,
    phase: Phase,
    files: Option<Files>,
    target: Option<u64>,
    hz_per_unit: u64,
    thermal: Guard,
    disabled: Option<Reason>,
    state: Reason,
    logged: bool,
    apply_error: Option<String>,
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
            phase: Phase::Idle,
            files,
            target: None,
            hz_per_unit,
            thermal,
            disabled,
            state: disabled.unwrap_or(Reason::Stopped),
            logged: false,
            apply_error: None,
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

    pub(super) fn state(&self) -> Reason {
        self.state
    }

    pub(super) fn saved(&self) -> Option<u64> {
        self.phase.previous()
    }

    pub(super) fn claim(&mut self, previous: u64) {
        self.phase = Phase::Prepared { previous };
    }

    pub(super) fn lowering(&self, target: u64) -> bool {
        self.saved().is_some() && self.target.is_some_and(|old| target < old)
    }

    pub(super) fn should_apply(&self, target: u64, now: Instant) -> bool {
        !self.attempted()
            || self.target != Some(target)
            || (self.state == Reason::ApplyFailed && now >= self.next_write)
    }

    pub(super) fn finish_apply(&mut self, target: u64, now: Instant, result: io::Result<()>) {
        let error = result.err().map(|error| error.to_string());
        if let Some(detail) = error.as_ref().filter(|_| error != self.apply_error || self.target != Some(target)) {
            eprintln!("oos-ui: clock_floor device={} target={target} error={detail}", self.name);
        }
        self.state = if error.is_some() { Reason::ApplyFailed } else { Reason::Applied };
        self.apply_error = error;
        self.target = Some(target);
        self.next_write = now + SAMPLE_PERIOD;
    }

    pub(super) fn restore_failed(&self) -> bool {
        matches!(self.phase, Phase::RestorePending { .. })
    }

    pub(super) fn release_failed_lowering(&mut self, port: &mut dyn Port) -> bool {
        if self.restore(port).is_ok() {
            true
        } else {
            self.state = Reason::RestoreFailed;
            false
        }
    }

    pub(super) fn release(
        &mut self,
        port: &mut dyn Port,
        inhibit: Option<Reason>,
        now: Instant,
        force: bool,
    ) -> Option<bool> {
        let immediate = force || self.attempted();
        if let Some(reason) = inhibit {
            self.state = reason;
        }
        if self.saved().is_none() || (!immediate && now < self.next_write) {
            return None;
        }
        self.next_write = now + SAMPLE_PERIOD;
        Some(self.restore(port).is_ok())
    }

    fn supports(available: &str, value: u64) -> bool {
        available.split_whitespace().any(|frequency| frequency.parse() == Ok(value))
    }

    pub(super) fn prepare(&self, io: &dyn Port, target: u64) -> io::Result<u64> {
        let files = self.files()?;
        let previous = match self.saved() {
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
        // Keep the persisted minimum, and invalidate verification before attempting a new write.
        self.phase = if self.restore_failed() {
            Phase::RestorePending { previous, verified: None }
        } else {
            Phase::Prepared { previous }
        };
        let minimum = previous.max(target);
        write_minimum(io, &self.files()?.minimum, minimum, "minimum read-back mismatch")?;
        self.phase = Phase::Raised { previous, minimum };
        if !self.logged {
            eprintln!("oos-ui: clock_floor device={} applied={minimum} previous={previous}", self.name);
            self.logged = true;
        }
        Ok(())
    }

    pub(super) fn restore(&mut self, io: &mut dyn Port) -> io::Result<()> {
        let Some(previous) = self.saved() else { return Ok(()) };
        let retry = self.restore_failed();
        self.phase = Phase::RestorePending { previous, verified: self.phase.verified() };
        let result = self.restore_value(io, previous);
        match &result {
            Err(error) if !retry => {
                eprintln!("oos-ui: clock_floor device={} running=false error={error}", self.name);
            }
            Ok(()) if retry => {
                eprintln!("oos-ui: clock_floor device={} restored={previous}", self.name);
            }
            _ => {}
        }
        result
    }

    fn restore_value(&mut self, io: &mut dyn Port, previous: u64) -> io::Result<()> {
        let files = self.files()?;
        if !Self::supports(&io.read(&files.available)?, previous) {
            return Err(io::Error::other("saved minimum is no longer available"));
        }
        write_minimum(io, &files.minimum, previous, "restore read-back mismatch")?;
        self.phase = Phase::Idle;
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
        if self.restore_failed() {
            let minimum = self.compare_minimum(port, target);
            (minimum.is_ok_and(|order| order != Ordering::Less), Reason::RestoreFailed)
        } else if recovery_failed {
            (false, Reason::RecoveryFailed)
        } else if self.state == Reason::Applied {
            match self.compare_minimum(port, target) {
                Ok(Ordering::Equal | Ordering::Greater) => (true, Reason::Applied),
                Ok(Ordering::Less) => (false, Reason::MinimumChanged),
                Err(reason) => (false, reason),
            }
        } else {
            (false, self.disabled.unwrap_or(self.state))
        }
    }

    fn compare_minimum(&self, port: &dyn Port, target: Option<u64>) -> Result<Ordering, Reason> {
        let Some(verified) = self.phase.verified() else { return Err(Reason::MinimumUnknown) };
        let minimum =
            self.files().and_then(|files| read_freq(port, &files.minimum)).map_err(|_| Reason::MinimumUnknown)?;
        Ok(minimum.cmp(&verified.max(target.unwrap_or_default())))
    }
}

fn write_minimum(port: &mut dyn Port, path: &str, value: u64, mismatch: &'static str) -> io::Result<()> {
    let write = port.write(path, value);
    match read_freq(port, path) {
        Ok(minimum) if minimum == value => Ok(()),
        Ok(_) => Err(match write {
            Ok(()) => io::Error::other(mismatch),
            Err(error) => io::Error::new(error.kind(), format!("{mismatch}: {error}")),
        }),
        Err(error) => write.and(Err(error)),
    }
}

fn read_freq(port: &dyn Port, path: &str) -> io::Result<u64> {
    port.read(path)?.trim().parse().map_err(io::Error::other)
}
