// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::acquisition::parse;
use oos_hal::dho::seq;
#[cfg(target_os = "linux")]
mod linux;

/// Roll mode test: configuration as for `run`, then the roll display chain, streaming for `--secs`, stop.
pub(crate) fn run(a: &[String]) -> Result<(), String> {
    let mut a = a.to_vec();
    let secs = match a.iter().position(|x| x == "--secs") {
        Some(i) => {
            let v: f64 = a.get(i + 1).ok_or("--secs needs a value")?.parse().map_err(|e| format!("--secs: {}", e))?;
            a.drain(i..=i + 1);
            v
        }
        None => 3.0,
    };
    let scan = match a.iter().position(|x| x == "--scan") {
        Some(i) => {
            a.remove(i);
            true
        }
        None => false,
    };
    let o = parse(&a)?;
    let tdiv_fs = seq::tdiv_fs(o.s.tdiv);
    let p = oos_hal::dho::hz::prep(o.s.mask, tdiv_fs, o.s.depth, 0);
    let rp = oos_hal::dho::roll::params(o.s.mask, tdiv_fs, o.s.depth, &p);
    let name = if scan { "scan" } else { "roll" };
    // scan: trigger at the screen centre -> 0x4058 = 0 (roll-mode.md 5, test E6)
    let slow_cfg = |sh: &mut oos_hal::dho::scu::Shadows| {
        if scan {
            oos_hal::dho::roll::config_scan(sh, &rp, p.memsel, 0)
        } else {
            oos_hal::dho::roll::config(sh, &rp, p.memsel)
        }
    };
    println!(
        "{}: {} Sa/s, memsel {}, k {}, f {}/{}, len {}, {:.3} us per point, chunk = {:.3} ms",
        name,
        p.rate,
        p.memsel,
        rp.k,
        rp.f_num,
        rp.f_den,
        rp.len,
        rp.spacing_fs as f64 / 1e9,
        rp.spacing_fs as f64 * 100.0 / rp.ratio as f64 / 1e12
    );
    if !o.execute {
        let mut sh = seq::State::handover(o.app_mask, None).sh;
        for (r, v) in slow_cfg(&mut sh).iter().chain(oos_hal::dho::roll::start(&mut sh, o.s.mask).iter()) {
            println!("  wr  {:#06x} {:#010x}", r, v);
        }
        eprintln!("oos-fpgad: dry run; add --execute");
        return Ok(());
    }
    #[cfg(target_os = "linux")]
    {
        linux::execute(&o, secs, scan, slow_cfg)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (secs, name);
        Err("roll --execute needs the unit (Linux)".into())
    }
}
