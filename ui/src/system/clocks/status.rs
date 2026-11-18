// SPDX-License-Identifier: GPL-2.0-or-later
use super::{Controller, FloorPhase, Kind};
use std::fmt;
use std::time::Instant;

#[derive(Clone)]
enum FloorReason {
    Idle,
    ConfigOff,
    DeviceUnavailable,
    NoController,
    RecoveryFailed,
    Thermal(String),
    NotEngaged,
}
impl fmt::Display for FloorReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Idle => "idle",
            Self::ConfigOff => "config-off",
            Self::DeviceUnavailable => "device-unavailable",
            Self::NoController => "no-controller",
            Self::RecoveryFailed => "recovery-failed",
            Self::Thermal(reason) => return reason.fmt(f),
            Self::NotEngaged => "not-engaged",
        })
    }
}

#[derive(Clone)]
enum FloorState {
    Engaged,
    RestorePending { raise_verified: bool },
    Disabled(FloorReason),
}
impl fmt::Display for FloorState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Engaged => f.write_str("engaged reason none"),
            Self::RestorePending { raise_verified } => {
                write!(f, "{} reason restore-pending", if *raise_verified { "engaged" } else { "disabled" })
            }
            Self::Disabled(reason) => write!(f, "disabled reason {reason}"),
        }
    }
}

#[derive(Clone, Copy, Default)]
pub(super) struct Sample {
    current_hz: Option<u64>,
    min_hz: Option<u64>,
    max_hz: Option<u64>,
    a53_khz: Option<u64>,
    a72_khz: Option<u64>,
    next: Option<Instant>,
}
impl Sample {
    fn read(controller: &Controller, now: Instant) -> Self {
        let frequency = |path: String| controller.port.read(&path).ok()?.trim().parse().ok();
        let minimum = |kind| {
            let (_, policy) = super::floor::CPU_POLICIES.iter().find(|(cpu, _)| *cpu == kind)?;
            frequency(super::DevicePaths::cpu(policy).minimum())
        };
        Self {
            current_hz: controller.gpu.as_ref().and_then(|gpu| frequency(gpu.attribute("cur_freq"))),
            min_hz: controller.gpu.as_ref().and_then(|gpu| frequency(gpu.minimum())),
            max_hz: controller.gpu.as_ref().and_then(|gpu| frequency(gpu.maximum())),
            a53_khz: minimum(Kind::CpuA53),
            a72_khz: minimum(Kind::CpuA72),
            next: Some(now + super::thermal::SAMPLE_PERIOD),
        }
    }
}

#[derive(Clone)]
pub(crate) struct Status {
    sample: Sample,
    a53: FloorState,
    a72: FloorState,
    gpu: FloorState,
}
impl Status {
    fn new(controller: &Controller) -> Self {
        Self {
            sample: controller.sample,
            a53: controller.floor_state(Kind::CpuA53),
            a72: controller.floor_state(Kind::CpuA72),
            gpu: controller.floor_state(Kind::Gpu),
        }
    }
    pub(crate) fn unavailable() -> Self {
        Self {
            sample: Sample::default(),
            a53: FloorState::Disabled(FloorReason::NoController),
            a72: FloorState::Disabled(FloorReason::NoController),
            gpu: FloorState::Disabled(FloorReason::NoController),
        }
    }
}
impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let frequency = |value: Option<u64>| value.map_or_else(|| "unknown".into(), |hz| hz.to_string());
        write!(
            f,
            " gpu_clock current_hz={} min_hz={} max_hz={} cpu_floor a53 {} min_khz={} a72 {} min_khz={} gpu_floor {}",
            frequency(self.sample.current_hz),
            frequency(self.sample.min_hz),
            frequency(self.sample.max_hz),
            self.a53,
            frequency(self.sample.a53_khz),
            self.a72,
            frequency(self.sample.a72_khz),
            self.gpu
        )
    }
}

impl Controller {
    pub(super) fn sample_readbacks(&mut self, changed: bool, now: Instant) {
        if changed || self.sample.next.is_none_or(|next| now >= next) {
            self.sample = Sample::read(self, now);
        }
    }

    fn floor_state(&self, kind: Kind) -> FloorState {
        let floor = self.floors.iter().find(|f| f.kind == kind);
        let allowed = self.allowed && kind.enabled(self.config);
        if let Some(floor) =
            floor.filter(|f| f.saved().is_some() && (!allowed || matches!(f.phase, FloorPhase::RestorePending { .. })))
        {
            return FloorState::RestorePending { raise_verified: floor.phase.raise_verified() };
        }
        let reason = if self.recovery_failed {
            FloorReason::RecoveryFailed
        } else if !kind.enabled(self.config) {
            FloorReason::ConfigOff
        } else if floor.is_none() {
            FloorReason::DeviceUnavailable
        } else if !self.requested {
            FloorReason::Idle
        } else if !allowed {
            self.thermal.reason().map_or(FloorReason::NotEngaged, |reason| FloorReason::Thermal(reason.to_string()))
        } else if floor.is_some_and(|f| matches!(f.phase, FloorPhase::Raised(_))) {
            return FloorState::Engaged;
        } else {
            FloorReason::NotEngaged
        };
        FloorState::Disabled(reason)
    }

    pub(super) fn status(&self) -> Status {
        Status::new(self)
    }
}
