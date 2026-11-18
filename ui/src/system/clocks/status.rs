// SPDX-License-Identifier: GPL-2.0-or-later
use super::{Controller, FloorPhase, Kind};
use std::fmt;

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

enum FloorState {
    Engaged,
    RestorePending,
    Disabled(FloorReason),
}
impl fmt::Display for FloorState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Engaged => f.write_str("engaged reason none"),
            Self::RestorePending => f.write_str("engaged reason restore-pending"),
            Self::Disabled(reason) => write!(f, "disabled reason {reason}"),
        }
    }
}

pub(crate) struct Status {
    current_hz: Option<u64>,
    min_hz: Option<u64>,
    max_hz: Option<u64>,
    a53: FloorState,
    a72: FloorState,
    a53_khz: Option<u64>,
    a72_khz: Option<u64>,
    gpu: FloorState,
}
impl Status {
    fn new(controller: &Controller) -> Self {
        let frequency = |path: String| controller.port.read(&path).ok()?.trim().parse().ok();
        let minimum = |kind| {
            let floor = controller.floors.iter().find(|f| f.kind == kind)?;
            frequency(floor.device.minimum())
        };
        Self {
            current_hz: controller.gpu.as_ref().and_then(|gpu| frequency(gpu.attribute("cur_freq"))),
            min_hz: controller.gpu.as_ref().and_then(|gpu| frequency(gpu.minimum())),
            max_hz: controller.gpu.as_ref().and_then(|gpu| frequency(gpu.maximum())),
            a53: controller.floor_state(Kind::CpuA53),
            a72: controller.floor_state(Kind::CpuA72),
            a53_khz: minimum(Kind::CpuA53),
            a72_khz: minimum(Kind::CpuA72),
            gpu: controller.floor_state(Kind::Gpu),
        }
    }
    pub(crate) fn unavailable() -> Self {
        Self {
            current_hz: None,
            min_hz: None,
            max_hz: None,
            a53: FloorState::Disabled(FloorReason::NoController),
            a72: FloorState::Disabled(FloorReason::NoController),
            a53_khz: None,
            a72_khz: None,
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
            frequency(self.current_hz),
            frequency(self.min_hz),
            frequency(self.max_hz),
            self.a53,
            frequency(self.a53_khz),
            self.a72,
            frequency(self.a72_khz),
            self.gpu
        )
    }
}

impl Controller {
    fn floor_state(&self, kind: Kind) -> FloorState {
        let floor = self.floors.iter().find(|f| f.kind == kind);
        let allowed = self.allowed && kind.enabled(self.config);
        if floor.is_some_and(|f| {
            matches!(f.phase, FloorPhase::RestorePending(_)) || (matches!(f.phase, FloorPhase::Raised(_)) && !allowed)
        }) {
            return FloorState::RestorePending;
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
