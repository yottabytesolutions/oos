// SPDX-License-Identifier: GPL-2.0-or-later

use crate::{board::HwShared, sysconf::Switches};
use oos_hal::dho::bus::{Bar1, Bus, FPGA_VERSION};
use std::fs::File;

pub(super) fn open_acquisition(switches: &Switches, shared: &HwShared) -> Result<(Bar1, File), String> {
    let mut bar = Bar1::open().map_err(|error| format!("/dev/xdma0_bypass: {error}"))?;
    let version = bar.rd(0x4000);
    crate::perfguard::deep::acq_lock(shared).fpga_version = version;
    if version != FPGA_VERSION {
        return Err(format!("unexpected FPGA version {version:#x}, not touching it"));
    }
    let c2h = File::open("/dev/xdma0_c2h_0").map_err(|error| format!("/dev/xdma0_c2h_0: {error}"))?;
    Ok((bar.with_batch_settle(switches.source.batch_settle()), c2h))
}
