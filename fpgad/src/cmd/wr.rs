// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::options::num;

pub(super) fn run(a: &[String]) -> Result<(), String> {
    use oos_hal::dho::bus::{Bar1, Bus};
    let has = |f: &str| a.iter().any(|x| x == f);
    let (off, v) = (
        num(a.first().ok_or("wr <offset> <value> --execute")?)?,
        num(a.get(1).ok_or("wr <offset> <value> --execute")?)?,
    );
    if off == 0x1008 {
        return Err("0x1008 refused (DDR status / BRAM fallback bit, cold-boot.md never-do list)".into());
    }
    if !has("--execute") {
        println!("dry run: would write {:#06x} <- {:#010x}; add --execute", off, v);
        return Ok(());
    }
    let mut bar = Bar1::open().map_err(|e| format!("/dev/xdma0_bypass: {}", e))?;
    bar.wr(off, v);
    println!("{:#06x} <- {:#010x}", off, v);
    Ok(())
}
