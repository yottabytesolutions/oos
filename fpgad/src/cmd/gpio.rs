// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::options;

pub(super) fn run(a: &[String]) -> Result<(), String> {
    let name = a.first().ok_or("gpio <line> <0|1> [--hold s]")?;
    let v = a.get(1).ok_or("gpio <line> <0|1> [--hold s]")? == "1";
    let hold = options::value(a, "--hold")
        .flatten()
        .map(|x| x.parse::<u64>().map_err(|e| e.to_string()))
        .transpose()?
        .unwrap_or(0);
    // generator relays and supply only on a unit with a generator (docs/models.md 4.2)
    oos_hal::dho::afg::line_allowed(&oos_hal::model::platform().caps, name, a.iter().any(|x| x == "--force"))?;
    let _l = oos_hal::dho::gpio::Line::output(name, v).map_err(|e| format!("{}: {}", name, e))?;
    println!("{} = {} (held {} s)", name, v as u8, hold);
    std::thread::sleep(std::time::Duration::from_secs(hold));
    Ok(())
}
