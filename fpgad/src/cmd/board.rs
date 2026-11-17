// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

//! Board subcommands: temperatures, beeper, over-temperature guard (oos_hal::dho::board, docs/protocols/board.md).

#[cfg(target_os = "linux")]
use oos_hal::dho::board;
use oos_hal::dho::board::Zone;
use oos_hal::dho::panel;

/// FPGA die temperature and VCCINT, if BAR1 is there and the FPGA answers.
pub(super) fn fpga_xadc() -> Option<(f32, f32)> {
    #[cfg(target_os = "linux")]
    {
        let mut bar = oos_hal::dho::bus::Bar1::open().ok()?;
        board::read_xadc(&mut bar)
    }
    #[cfg(not(target_os = "linux"))]
    None
}

/// One line of readings.
pub(super) fn line(zones: &[Zone], fpga: Option<(f32, f32)>) -> String {
    let mut s: Vec<String> = zones.iter().map(|z| format!("{} {:.1} C", z.name, z.temp)).collect();
    match fpga {
        Some((t, v)) => s.push(format!("fpga {:.1} C, vccint {:.3} V", t, v)),
        None => s.push("fpga -".into()),
    }
    s.join(", ")
}

/// Power off through the front-panel MCU: sync, then `FA 05 01` twice like the stock system.
pub(super) fn power_off() -> Result<(), String> {
    #[cfg(unix)]
    // SAFETY: sync has no arguments and imposes no memory invariants.
    unsafe {
        libc::sync()
    };
    let mut f = std::fs::OpenOptions::new().write(true).open("/dev/ttyS0").map_err(|e| format!("/dev/ttyS0: {}", e))?;
    use std::io::Write;
    for _ in 0..2 {
        f.write_all(&panel::POWER_OFF).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readings_line() {
        let z = vec![Zone { name: "cpu-thermal".into(), temp: 45.25 }];
        assert_eq!(line(&z, Some((51.0, 0.998))), "cpu-thermal 45.2 C, fpga 51.0 C, vccint 0.998 V");
        assert_eq!(line(&[], None), "fpga -");
    }
}
