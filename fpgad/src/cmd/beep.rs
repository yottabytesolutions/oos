// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use oos_hal::dho::board::{Beep, Beeper};
use std::path::Path;

pub(super) fn run(a: &[String]) -> Result<(), String> {
    let b = match a.first().map(|s| s.as_str()) {
        Some("once") | None => Beep::Once,
        Some("long") => Beep::Long,
        Some(x) => return Err(format!("beep once|long, not {}", x)),
    };
    let bp = Beeper::open(Path::new("/sys/class/pwm")).map_err(|e| format!("beeper: {}", e))?;
    bp.play(b).map_err(|e| format!("beeper: {}", e))?;
    Ok(())
}
