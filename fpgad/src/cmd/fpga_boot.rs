// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::options::{self, num};
use oos_hal::bootstatus;

pub(super) fn run(a: &[String]) -> Result<(), String> {
    let slot = options::value(a, "--slot").flatten().map(num).transpose()?.unwrap_or(0x40_0000);
    let progress = options::value(a, "--progress").flatten().map(str::to_owned);
    oos_hal::dho::boot::io::fpga_boot(slot).map_err(|e| {
        if let Some(p) = &progress {
            // the cold boot sends it again (rcS runs it with --booted, which falls back)
            bootstatus::append(p, "warn", "FPGA image", &format!("early BOOT command failed ({}), retried later", e));
        }
        format!("FPGA boot: {}", e)
    })?;
    let t = oos_hal::dho::boot::io::uptime();
    std::fs::write(BOOTED_FILE, format!("{:.2}\n", t)).map_err(|e| format!("{}: {}", BOOTED_FILE, e))?;
    if let Some(p) = &progress {
        bootstatus::append(p, "run", "FPGA image", "loading the image from flash");
    }
    println!("BOOT {:#x} sent at uptime {:.2} s", slot, t);
    Ok(())
}

/// Uptime of the BOOT command sent by `oos-fpgad fpga-boot`.
pub(super) const BOOTED_FILE: &str = "/tmp/oos-fpga-booted";
