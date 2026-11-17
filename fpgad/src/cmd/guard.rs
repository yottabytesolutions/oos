// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::{
    board::{fpga_xadc, line, power_off},
    options,
};
use oos_hal::dho::board::{self, Alarm, Beep, Beeper, Guard};
use std::path::Path;

pub(super) fn run(a: &[String]) -> Result<(), String> {
    let thermal = Path::new("/sys/class/thermal");
    let period = options::value(a, "--period")
        .flatten()
        .map(|x| x.parse::<f32>().map_err(|e| e.to_string()))
        .transpose()?
        .unwrap_or(1.0);
    let arm = a.iter().any(|x| x == "--power-off");
    let beeper = Beeper::open(Path::new("/sys/class/pwm")).ok();
    let mut g = Guard::default();
    eprintln!(
        "oos-fpgad guard: period {} s, limits SoC {} C / FPGA {} C, power off {}",
        period,
        board::SOC_LIMIT,
        board::FPGA_LIMIT,
        if arm { "ARMED" } else { "not armed (log only)" }
    );
    loop {
        let (z, x) = (board::thermal_zones(thermal), fpga_xadc());
        let ev = g.feed(board::is_hot(&z, x.map(|v| v.0)));
        if let Some(ev) = ev {
            eprintln!("guard: {:?} after {} hot periods: {}", ev, g.count(), line(&z, x));
            match ev {
                Alarm::Warn => {
                    if let Some(b) = &beeper {
                        let _ = b.play(Beep::Long);
                    }
                }
                Alarm::PowerOff if arm => return power_off(),
                _ => {}
            }
        }
        std::thread::sleep(std::time::Duration::from_secs_f32(period));
    }
}
