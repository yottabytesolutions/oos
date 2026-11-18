// SPDX-License-Identifier: GPL-2.0-or-later
use super::{Config, Port};
use std::fmt;
use std::io;
use std::str::FromStr;
use std::sync::Arc;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    CpuA53,
    CpuA72,
    Gpu,
}
impl Kind {
    pub(super) fn enabled(self, config: Config) -> bool {
        match self {
            Self::CpuA53 | Self::CpuA72 => config.cpu,
            Self::Gpu => config.gpu,
        }
    }
}
impl FromStr for Kind {
    type Err = io::Error;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        match name {
            "cpu-a53" => Ok(Self::CpuA53),
            "cpu" | "cpu-a72" => Ok(Self::CpuA72),
            "gpu" => Ok(Self::Gpu),
            _ => Err(io::Error::other("invalid clock recovery receipt")),
        }
    }
}
impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::CpuA53 => "cpu-a53",
            Self::CpuA72 => "cpu-a72",
            Self::Gpu => "gpu",
        })
    }
}

pub(super) struct DevicePaths {
    directory: String,
    minimum_attribute: &'static str,
    maximum_attribute: &'static str,
    available_attribute: &'static str,
}
impl DevicePaths {
    pub(super) fn cpu(policy: &str) -> Self {
        Self {
            directory: format!("devices/system/cpu/cpufreq/{policy}"),
            minimum_attribute: "scaling_min_freq",
            maximum_attribute: "scaling_max_freq",
            available_attribute: "scaling_available_frequencies",
        }
    }
    pub(super) fn gpu(directory: &str) -> Self {
        Self {
            directory: directory.into(),
            minimum_attribute: "min_freq",
            maximum_attribute: "max_freq",
            available_attribute: "available_frequencies",
        }
    }
    pub(super) fn attribute(&self, name: &str) -> String {
        format!("{}/{name}", self.directory)
    }
    pub(super) fn minimum(&self) -> String {
        self.attribute(self.minimum_attribute)
    }
    pub(super) fn maximum(&self) -> String {
        self.attribute(self.maximum_attribute)
    }
    pub(super) fn available(&self) -> String {
        self.attribute(self.available_attribute)
    }
}

#[derive(Clone, Copy)]
pub(super) enum FloorPhase {
    Idle,
    // Receipt persisted; hardware is unverified, including after failed writes/read-backs.
    Prepared(u64),
    // Only a valid raise read-back enters Raised. Every non-idle state owns the same receipt.
    Raised(u64),
    // Any restore attempt enters RestorePending; only confirmed restoration enters Idle.
    RestorePending(u64),
}

pub(super) struct Floor {
    pub(super) kind: Kind,
    pub(super) device: Arc<DevicePaths>,
    pub(super) phase: FloorPhase,
    pub(super) logged: bool,
    raise_error: Option<String>,
}
impl Floor {
    pub(super) fn new(kind: Kind, device: Arc<DevicePaths>) -> Self {
        Self { kind, device, phase: FloorPhase::Idle, logged: false, raise_error: None }
    }

    pub(super) fn saved(&self) -> Option<u64> {
        match self.phase {
            FloorPhase::Idle => None,
            FloorPhase::Prepared(previous) | FloorPhase::Raised(previous) | FloorPhase::RestorePending(previous) => {
                Some(previous)
            }
        }
    }

    pub(super) fn cpu(kind: Kind, policy: &str) -> Self {
        Self::new(kind, Arc::new(DevicePaths::cpu(policy)))
    }

    pub(super) fn supports(&self, io: &dyn Port, value: u64) -> io::Result<bool> {
        Ok(io
            .read(&self.device.available())?
            .split_whitespace()
            .filter_map(|s| s.parse::<u64>().ok())
            .any(|v| v == value))
    }

    fn requested(&self, io: &dyn Port, previous: u64) -> io::Result<u64> {
        if self.kind == Kind::Gpu {
            return Ok(previous.max(600_000_000));
        }
        let maximum = io.read(&self.device.maximum())?.trim().parse::<u64>().map_err(io::Error::other)?;
        io.read(&self.device.available())?
            .split_whitespace()
            .filter_map(|value| value.parse::<u64>().ok())
            .filter(|value| *value > 0 && *value <= maximum)
            .max()
            .ok_or_else(|| io::Error::other("no available frequency within policy maximum"))
    }

    pub(super) fn prepare(&self, io: &dyn Port) -> io::Result<u64> {
        let previous = io.read(&self.device.minimum())?.trim().parse::<u64>().map_err(io::Error::other)?;
        let applied = self.requested(io, previous)?;
        if !self.supports(io, applied)? || !self.supports(io, previous)? {
            return Err(io::Error::other("minimum is not in available frequencies"));
        }
        Ok(previous)
    }

    pub(super) fn raise(&mut self, io: &mut dyn Port) -> io::Result<()> {
        let Some(previous) = self.saved() else { return Ok(()) };
        let applied = self.requested(io, previous)?;
        // The controller persisted this minimum before calling raise.
        io.write(&self.device.minimum(), applied)?;
        let actual = io.read(&self.device.minimum())?.trim().parse::<u64>().map_err(io::Error::other)?;
        let valid_clamp = self.kind != Kind::Gpu && actual > 0 && actual < applied && self.supports(io, actual)?;
        if actual != applied && !valid_clamp {
            return Err(io::Error::other("minimum read-back mismatch"));
        }
        self.phase = FloorPhase::Raised(previous);
        if !self.logged {
            eprintln!("oos-ui: clock_floor device={} applied={} previous={}", self.kind, actual, previous);
            self.logged = true;
        }
        Ok(())
    }

    pub(super) fn report_raise(&mut self, result: io::Result<()>) {
        let error = result.err().map(|error| error.to_string());
        if error != self.raise_error {
            if let Some(error) = &error {
                eprintln!("oos-ui: clock_floor device={} running=true error={error}", self.kind);
            }
            self.raise_error = error;
        }
    }

    pub(super) fn restore(&mut self, io: &mut dyn Port) -> io::Result<()> {
        let Some(previous) = self.saved() else {
            return Ok(());
        };
        let retry = matches!(self.phase, FloorPhase::RestorePending(_));
        self.phase = FloorPhase::RestorePending(previous);
        let result = self.restore_value(io, previous);
        match &result {
            Err(error) if !retry => {
                eprintln!("oos-ui: clock_floor device={} running=false error={error}", self.kind);
            }
            Ok(()) if retry => {
                eprintln!("oos-ui: clock_floor device={} restored={previous}", self.kind);
            }
            _ => {}
        }
        result
    }

    fn restore_value(&mut self, io: &mut dyn Port, previous: u64) -> io::Result<()> {
        if !self.supports(io, previous)? {
            return Err(io::Error::other("saved minimum is no longer available"));
        }
        io.write(&self.device.minimum(), previous)?;
        if io.read(&self.device.minimum())?.trim() != previous.to_string() {
            return Err(io::Error::other("restore read-back mismatch"));
        }
        self.phase = FloorPhase::Idle;
        Ok(())
    }
}
