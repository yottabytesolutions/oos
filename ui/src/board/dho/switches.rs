// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! The DHO's switches in system.conf and the environment (`sysconf::Switches`), parsed once at start.

use super::hw;
use crate::drawpipe::conf_value;
use crate::sysconf::switches::{flag, text, Env};

/// The board's part of `sysconf::Switches`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Switches {
    /// OOS_HWAVG=hold: the FPGA average is read as a cumulative average of the count that is shown when complete and
    /// then cleared, else running (spec 01, 6.1; the hardware test in docs/fork/unit-dho804.md decides)
    pub avg_hold: bool,
    /// `cal.core_offset = tight` (docs/fork/core-offset.md): the core offsets of every ADC mode used are matched to
    /// each other at its first configuration, and kept in OOS's own cal_adc.hex
    pub core_offset_tight: bool,
    /// OOS_FRAME_HEADER set (trigger.md 8): frames are read with the 16-byte header of the stock live loop and every
    /// frame's header is logged to /tmp/oos-hdr.log next to where the edge actually is
    pub header_probe: bool,
    /// the trigger status from the SCU status bits (docs/fork/trigger-lockin.md); `trig.hw_status = off` or
    /// OOS_TRIG_HW_STATUS=0 takes it from the software refinement instead
    pub trig_hw_status: bool,
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
        let status = flag(env, "OOS_TRIG_HW_STATUS");
        Switches {
            avg_hold: text(env, "OOS_HWAVG").is_some_and(|v| v == "hold"),
            core_offset_tight: hw::core_offset_from(conf),
            header_probe: env("OOS_FRAME_HEADER").is_some(),
            trig_hw_status: conf_value(status, conf, "trig.hw_status").as_deref() != Some("off"),
            dvm: hw::dvm_hw_from(conf),
            mask: hw::mask_hw_from(conf),
            zone: hw::zone_hw_from(conf),
            prearm: hw::prearm_from(env("OOS_PREARM").is_some(), conf),
            zone_batch: hw::zone_batch_from(settings),
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
