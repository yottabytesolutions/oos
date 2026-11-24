// SPDX-License-Identifier: GPL-2.0-or-later
use std::fmt::Write;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum Reason {
    Applied,
    Stopped,
    Disabled,
    Absent,
    NoCoolingBinding,
    CoolingBindingError,
    Thermal,
    SensorError,
    RecoveryFailed,
    ApplyFailed,
    RestoreFailed,
    MinimumUnknown,
    MinimumChanged,
}
impl Reason {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Applied => "none",
            Self::Stopped => "stopped",
            Self::Disabled => "disabled",
            Self::Absent => "absent",
            Self::NoCoolingBinding => "no_cooling_binding",
            Self::CoolingBindingError => "cooling_binding_error",
            Self::Thermal => "thermal",
            Self::SensorError => "sensor_error",
            Self::RecoveryFailed => "recovery_failed",
            Self::ApplyFailed => "apply_failed",
            Self::RestoreFailed => "restore_failed",
            Self::MinimumUnknown => "minimum_unknown",
            Self::MinimumChanged => "minimum_changed",
        }
    }
}

/// Read-back state for one clock floor. Frequencies are always in Hz.
pub(super) struct Status {
    device: &'static str,
    requested_hz: u64,
    current_hz: Option<u64>,
    applied: bool,
    reason: Reason,
}
impl Status {
    pub(super) fn absent() -> [Self; 2] {
        [Self::new("cpu", 0, None, false, Reason::Absent), Self::new("gpu", 0, None, false, Reason::Absent)]
    }

    pub(super) fn new(
        device: &'static str,
        requested_hz: u64,
        current_hz: Option<u64>,
        applied: bool,
        reason: Reason,
    ) -> Self {
        Self { device, requested_hz, current_hz, applied, reason }
    }

    pub(super) fn write_scpi(&self, text: &mut String) {
        let Self { device, requested_hz, current_hz, applied, reason } = *self;
        let _ = write!(
            text,
            " clock.{device}.requested_hz={requested_hz} clock.{device}.applied={} clock.{device}.current_hz=",
            u8::from(applied)
        );
        if let Some(hz) = current_hz {
            let _ = write!(text, "{hz}");
        } else {
            text.push_str("unknown");
        }
        let _ = write!(text, " clock.{device}.reason={}", reason.as_str());
    }
}
