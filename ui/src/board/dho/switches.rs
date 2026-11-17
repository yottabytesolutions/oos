// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! The DHO's switches in system.conf and the environment (`sysconf::Switches`), parsed once at start.

use super::hw;
use crate::sysconf::switches::{text, Env};

/// The frame-header log (`OOS_FRAME_HEADER`, trigger.md 8): unset, set (every frame), or `off` (only the frames
/// whose edge is off the trigger point, and a count of all frames every 10,000; docs/protocols/acquisition-start.md,
/// "Frames 0 and 1 of a batch").
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HeaderProbe {
    #[default]
    Off,
    All,
    OffTrigger,
}

impl HeaderProbe {
    fn of(v: Option<std::ffi::OsString>) -> Self {
        match v {
            None => Self::Off,
            Some(v) if v == "off" => Self::OffTrigger,
            Some(_) => Self::All,
        }
    }

    /// The log of a read that carries headers on its own (segments): every frame.
    pub fn of_header(header: bool) -> Self {
        if header {
            Self::All
        } else {
            Self::Off
        }
    }

    /// Frames are read with their header.
    pub fn on(self) -> bool {
        self != Self::Off
    }
}

/// The board's part of `sysconf::Switches`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Switches {
    /// OOS_HWAVG=hold: the FPGA average is read as a cumulative average of the count that is shown when complete and
    /// then cleared, else running (spec 01, 6.1; the hardware test in docs/fork/unit-dho804.md decides)
    pub avg_hold: bool,
    /// `cal.core_offset = tight` (docs/fork/core-offset.md): the core offsets of every ADC mode used are matched to
    /// each other at its first configuration, and kept in OOS's own cal_adc.hex
    pub core_offset_tight: bool,
    /// OOS_FRAME_HEADER set (trigger.md 8): frames are read with the 16-byte header of the stock live loop and their
    /// headers are logged to /tmp/oos-hdr.log next to where the edge actually is
    pub header_probe: HeaderProbe,
    /// `dvm.hw = on`: the DVM reads the FPGA's DVM block (docs/fork/dvm.md)
    pub dvm: bool,
    /// `mask.hw = on`: the mask test runs in the FPGA's mask unit (docs/fork/hwmask.md)
    pub mask: bool,
    /// `zone.hw = on`: the zone trigger qualifies frames in the FPGA (docs/fork/hwzone.md)
    pub zone: bool,
    /// `acq.prearm = on` or OOS_PREARM set: arm the next batch before the decode of the current one (`seq::prearm`;
    /// docs/fork/specs/10-transfer-efficiency.md)
    pub prearm: bool,
    /// `zone.batch` of the settings file: frames per batch while the zone trigger runs (docs/fork/hwzone.md "Rate")
    pub zone_batch: u32,
    /// `acq.frame0 = keep` in the settings file turns it off: frame 0 of a batch of several frames, and the first
    /// short record after a change of settings, are left out (upstream issue #5)
    pub frame0_drop: bool,
    /// `trig.serial_hw = on` in the settings file: the UART protocol trigger runs in the TPU's receiver
    /// (docs/fork/hwuart.md)
    pub serial_hw: bool,
    /// the comparator mapping of the mask and zone units from the settings file's `mask.map.*` lines, the start value
    /// of the mapping under test (`hw::mask_map_set`)
    pub mask_map: oos_hal::dho::mask::Map,
}

impl Switches {
    /// The switches from an environment lookup, the fork's system.conf text and the settings file alone.
    pub fn parse(env: Env, conf: &str, settings: &str) -> Self {
        Switches {
            avg_hold: text(env, "OOS_HWAVG").is_some_and(|v| v == "hold"),
            core_offset_tight: hw::core_offset_from(conf),
            header_probe: HeaderProbe::of(env("OOS_FRAME_HEADER")),
            dvm: hw::dvm_hw_from(conf),
            mask: hw::mask_hw_from(conf),
            zone: hw::zone_hw_from(conf),
            prearm: hw::prearm_from(env("OOS_PREARM").is_some(), conf),
            zone_batch: hw::zone_batch_from(settings),
            frame0_drop: hw::frame0_drop_from(settings),
            serial_hw: hw::serial_hw_from(settings),
            mask_map: hw::mask_map_from(settings),
        }
    }

    /// How the FPGA average is read.
    pub fn avg_kind(&self) -> oos_hal::dho::avg::Kind {
        if self.avg_hold {
            oos_hal::dho::avg::Kind::Hold
        } else {
            oos_hal::dho::avg::Kind::Running
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Break it catches: OOS_FRAME_HEADER=off read as "log every frame", or read again in the decode loop (1dc4009c).
    #[test]
    fn the_header_log_has_its_mode_from_the_switch() {
        let mode = |v: Option<&str>| HeaderProbe::of(v.map(std::ffi::OsString::from));
        assert_eq!(mode(None), HeaderProbe::Off);
        assert_eq!(mode(Some("1")), HeaderProbe::All);
        assert_eq!(mode(Some("off")), HeaderProbe::OffTrigger);
        assert!(!HeaderProbe::Off.on() && HeaderProbe::All.on() && HeaderProbe::OffTrigger.on());
    }

    // Break it catches: the mapping under test taken from the build machine's settings file instead of the switches.
    #[test]
    fn mask_mapping_comes_from_the_settings_text() {
        let none = Switches::parse(&|_| None, "", "");
        assert_eq!(none.mask_map, oos_hal::dho::mask::Map::default());
        let mut want = oos_hal::dho::mask::Map::default();
        want.set("cols=stock").unwrap();
        assert_eq!(Switches::parse(&|_| None, "", "mask.map.cols = stock\n").mask_map, want);
        assert_eq!(
            Switches::parse(&|_| None, "mask.map.cols = stock\n", "").mask_map,
            none.mask_map,
            "settings file only"
        );
    }
}
