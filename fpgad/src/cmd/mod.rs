// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

mod acquisition;
#[cfg(any(target_os = "linux", test))]
mod adc;
pub(crate) mod afg;
mod beep;
mod board;
#[cfg(target_os = "linux")]
mod cold_boot;
mod compress_check;
#[cfg(target_os = "linux")]
mod fpga_boot;
#[cfg(target_os = "linux")]
mod gpio;
mod guard;
pub(crate) mod la;
pub(crate) mod model;
pub(crate) mod options;
pub(crate) mod plan;
mod pll;
#[cfg(target_os = "linux")]
mod rd;
pub(crate) mod roll;
pub(crate) mod run;
pub(crate) mod selfcal;
#[cfg(any(target_os = "linux", test))]
mod status;
mod temp;
#[cfg(target_os = "linux")]
mod wr;

use oos_hal::dho::seq::Settings;

struct Opt {
    s: Settings,
    app_mask: u32,
    #[cfg(target_os = "linux")]
    out: String,
    execute: bool,
    no_read: bool,
    #[cfg(target_os = "linux")]
    raw_too: bool,
}

pub(crate) fn tool(cmd: &str, a: &[String]) -> Result<(), String> {
    let has = |f: &str| a.iter().any(|x| x == f);
    let dir = std::env::var("OOS_CAL_DIR").unwrap_or_else(|_| "/rigol/data".into());
    if cmd == "cold-boot" && !has("--execute") {
        let cal = oos_hal::dho::boot::CalFiles::load(&dir);
        print!("{}", oos_hal::dho::boot::dry_run(&cal, &oos_hal::dho::boot::Options::default()));
        eprintln!("oos-fpgad: dry run against the unit model; add --execute on the unit");
        return Ok(());
    }
    if cmd == "pll" {
        if let Some(t) = pll::dry(a)? {
            print!("{}", t);
            return Ok(());
        }
    }
    crate::boot_guard::check(cmd, has("--force"), |name| std::path::Path::new("/sys/module").join(name).exists())?;
    #[cfg(target_os = "linux")]
    match cmd {
        "status" => status::run(a),
        "gpio" => gpio::run(a),
        "pll" => pll::run(a),
        "rd" => rd::run(a),
        "adc" => adc::run(a),
        "wr" => wr::run(a),
        "fpga-boot" => fpga_boot::run(a),
        "cold-boot" => cold_boot::run(a),
        _ => unreachable!(),
    }
    #[cfg(not(target_os = "linux"))]
    Err(format!("{} needs the unit (Linux)", cmd))
}

pub(crate) fn board(cmd: &str, args: &[String]) -> Result<(), String> {
    match cmd {
        "temp" => {
            temp::run();
            Ok(())
        }
        "beep" => beep::run(args),
        "guard" => guard::run(args),
        _ => unreachable!(),
    }
}

pub(crate) fn compress_check(args: &[String]) -> Result<(), String> {
    compress_check::run(args).map(|text| print!("{}", text))
}
