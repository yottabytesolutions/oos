// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::options::num;

pub(super) fn run(a: &[String]) -> Result<(), String> {
    use oos_hal::dho::bus::{Bar1, Bus};
    let off = num(a.first().ok_or("rd <offset> [count]")?)?;
    let n = a.get(1).map(|x| num(x)).transpose()?.unwrap_or(1);
    let mut bar = Bar1::open().map_err(|e| format!("/dev/xdma0_bypass: {}", e))?;
    for k in 0..n {
        let o = off + 4 * k;
        println!("{:#06x} = {:#010x}", o, bar.rd(o));
    }
    Ok(())
}
