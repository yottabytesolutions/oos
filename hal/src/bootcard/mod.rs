// SPDX-License-Identifier: GPL-2.0-or-later
//! Boot SD card diagnostics shared by the start-up check and UI.

mod probe;
mod record;
mod timing;

pub use probe::detect;
pub use record::{read_state, write_state, STATE_PATH};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Mode {
    Legacy,
    Hs,
    Sdr12,
    Sdr25,
    Sdr50,
    Ddr50,
    Sdr104,
    #[default]
    Unknown,
}

impl Mode {
    const ALL: [Self; 8] =
        [Self::Legacy, Self::Hs, Self::Sdr12, Self::Sdr25, Self::Sdr50, Self::Ddr50, Self::Sdr104, Self::Unknown];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Legacy => "legacy",
            Self::Hs => "hs",
            Self::Sdr12 => "sdr12",
            Self::Sdr25 => "sdr25",
            Self::Sdr50 => "sdr50",
            Self::Ddr50 => "ddr50",
            Self::Sdr104 => "sdr104",
            Self::Unknown => "unknown",
        }
    }

    /// A fixed display name for the timing, safe to show without copying kernel text.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Legacy => "Default speed",
            Self::Hs => "High speed",
            Self::Sdr12 => "SDR12",
            Self::Sdr25 => "SDR25",
            Self::Sdr50 => "SDR50",
            Self::Ddr50 => "DDR50",
            Self::Sdr104 => "SDR104",
            Self::Unknown => "Unknown",
        }
    }

    /// Nominal throughput in MB/s on a four-bit bus.
    fn rate(self) -> u8 {
        match self {
            Self::Unknown => 0,
            Self::Legacy | Self::Sdr12 => 12,
            Self::Hs | Self::Sdr25 => 25,
            Self::Sdr50 | Self::Ddr50 => 50,
            Self::Sdr104 => 104,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CardState {
    Ok,
    Degraded,
    Worse,
    /// Reserved for a card with no prior observation at the expected mode.
    Unverified,
    Unknown,
}

impl CardState {
    const ALL: [Self; 5] = [Self::Ok, Self::Degraded, Self::Worse, Self::Unverified, Self::Unknown];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Degraded => "degraded",
            Self::Worse => "worse",
            Self::Unverified => "unverified",
            Self::Unknown => "unknown",
        }
    }

    fn compare(mode: Mode, expected: Mode) -> Self {
        if mode == Mode::Unknown || expected == Mode::Unknown {
            Self::Unknown
        } else if mode.rate() < Mode::Hs.rate() {
            Self::Worse
        } else if mode.rate() < expected.rate() {
            Self::Degraded
        } else {
            Self::Ok
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Reason {
    None,
    BootHost,
    DeviceTree,
    DebugfsUnavailable,
    IosUnavailable,
    MalformedTiming,
    UnknownTiming,
    StateUnavailable,
}

impl Reason {
    const ALL: [Self; 8] = [
        Self::None,
        Self::BootHost,
        Self::DeviceTree,
        Self::DebugfsUnavailable,
        Self::IosUnavailable,
        Self::MalformedTiming,
        Self::UnknownTiming,
        Self::StateUnavailable,
    ];

    fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::BootHost => "boot-host-unavailable",
            Self::DeviceTree => "device-tree-unavailable",
            Self::DebugfsUnavailable => "debugfs-unavailable",
            Self::IosUnavailable => "ios-unavailable",
            Self::MalformedTiming => "malformed-timing",
            Self::UnknownTiming => "unknown-timing",
            Self::StateUnavailable => "state-unavailable",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Status {
    mode: Mode,
    expected: Mode,
    state: CardState,
    reason: Reason,
}

impl Status {
    fn from_probe(mode: Result<Mode, Reason>, expected: Result<Mode, Reason>) -> Self {
        let reason = mode.err().or(expected.err()).unwrap_or(Reason::None);
        let (mode, expected) = (mode.unwrap_or_default(), expected.unwrap_or_default());
        Self { mode, expected, state: CardState::compare(mode, expected), reason }
    }

    pub const fn mode(self) -> Mode {
        self.mode
    }

    pub const fn expected(self) -> Mode {
        self.expected
    }

    pub const fn state(self) -> CardState {
        self.state
    }
}

impl Default for Status {
    fn default() -> Self {
        Self::from_probe(Err(Reason::StateUnavailable), Ok(Mode::Unknown))
    }
}
