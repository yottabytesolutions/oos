// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::options;
use oos_hal::dho::afg;
pub(crate) fn run(a: &[String]) -> Result<(), String> {
    let exec = a.iter().any(|x| x == "--execute");
    let dir = std::env::var("OOS_CAL_DIR").unwrap_or_else(|_| "/rigol/data".into());
    if !matches!(a.first().map(String::as_str), Some("sine" | "square" | "ramp" | "pulse" | "noise" | "dc" | "off")) {
        return Err("afg sine|off, la status|thr".into());
    }
    let ops = operations(a, &dir)?;
    if !exec {
        let n = ops.iter().filter(|o| matches!(o, afg::Op::Bar1 { off: 0x3038, .. })).count();
        for o in ops.iter().filter(|o| !matches!(o, afg::Op::Bar1 { off: 0x3038, .. })) {
            println!("  {:?}", o);
        }
        println!("  (+ {} table samples to 0x3038)", n);
        eprintln!("oos-fpgad: dry run; add --execute (GI/GO up to 10 Vpp: nothing sensitive connected)");
        return Ok(());
    }
    #[cfg(target_os = "linux")]
    {
        use oos_hal::dho::bus::{Bar1, Bus, FPGA_VERSION};
        let mut bar = Bar1::open().map_err(|e| format!("/dev/xdma0_bypass: {}", e))?;
        if bar.rd(0x4000) != FPGA_VERSION {
            return Err("unexpected FPGA version, not touching it".into());
        }
        let open = if a.iter().any(|x| x == "--force") { afg::Io::open_forced } else { afg::Io::open };
        let mut io = open().map_err(|e| format!("AFG GPIO: {}", e))?;
        io.apply(&mut bar, &ops);
        println!("done; relays now {:#04x}. The lines stay as set while this process runs: Ctrl-C ends it (output keeps its state).", io.relays());
        if a[0] != "off" {
            // keep the GPIO handles (relays/supply) for a while
            std::thread::sleep(std::time::Duration::from_secs(number(a, "--hold")?.unwrap_or(60.0) as u64));
        }
    }
    Ok(())
}

fn number(a: &[String], k: &str) -> Result<Option<f64>, String> {
    options::value(a, k).flatten().map(|v| v.parse::<f64>().map_err(|e| format!("{}: {}", k, e))).transpose()
}

fn operations(a: &[String], dir: &str) -> Result<Vec<afg::Op>, String> {
    let get = |k| number(a, k);
    let ops = if a[0] != "off" {
        let (f, amp, off) =
            (get("--freq")?.unwrap_or(1000.0), get("--ampl")?.unwrap_or(1.0), get("--offset")?.unwrap_or(0.0));
        let wave = wave(a)?;
        let cal = afg::Cal::load(dir);
        let (ac, oc, g) =
            afg::codes(cal.as_ref(), amp * 1e9, off * 1e9, f * 1e6, wave == afg::Wave::Noise, wave == afg::Wave::Dc);
        println!(
            "afg {:?} {} Hz {} Vpp {} V: relays {:#04x}, 0x3070 {:#06x}, 0x3078 {:#06x}, increment {:#x} ({})",
            wave,
            f,
            amp,
            off,
            g.code,
            0xffff - ac,
            0xffff - oc,
            afg::increment(f * 1e6),
            if cal.is_some() { "cal_afg.hex" } else { "no cal file" }
        );
        let md = modulation(a, f)?;
        if md != afg::Mod::None {
            println!("modulation {:?}", md);
        }
        afg::start_mod(cal.as_ref(), wave, f * 1e6, amp * 1e9, off * 1e9, 0x00, md)
    } else {
        afg::stop()
    };
    Ok(ops)
}

fn wave(a: &[String]) -> Result<afg::Wave, String> {
    let get = |k| number(a, k);
    let duty = get("--duty")?.unwrap_or(50.0) / 100.0;
    let wave = match a[0].as_str() {
        "square" => afg::Wave::Square { duty },
        "ramp" => afg::Wave::Ramp { symmetry: get("--symmetry")?.unwrap_or(50.0) / 100.0 },
        "pulse" => afg::Wave::Pulse { duty, rise: get("--edge")?.unwrap_or(0.0), fall: get("--edge")?.unwrap_or(0.0) },
        "noise" => afg::Wave::Noise,
        "dc" => afg::Wave::Dc,
        _ => afg::Wave::Sine,
    };
    Ok(wave)
}

fn modulation(a: &[String], f: f64) -> Result<afg::Mod, String> {
    let get = |k| number(a, k);
    // --mod am|fm|pm --mod-rate Hz --depth % (AM) --dev Hz (FM) / degrees (PM) --mod-shape sine|square|triangle|up|down
    let shape = match options::value(a, "--mod-shape").flatten() {
        Some("square") => afg::ModShape::Square,
        Some("triangle") => afg::ModShape::Triangle,
        Some("up") => afg::ModShape::UpRamp,
        Some("down") => afg::ModShape::DownRamp,
        Some("noise") => afg::ModShape::Noise,
        _ => afg::ModShape::Sine,
    };
    let rate = get("--mod-rate")?.unwrap_or(100.0);
    let md = match options::value(a, "--mod").flatten() {
        Some("am") => afg::Mod::Am { rate, depth: get("--depth")?.unwrap_or(100.0) / 100.0, shape },
        Some("fm") => afg::Mod::Fm { rate, dev: get("--dev")?.unwrap_or(f / 10.0), shape },
        Some("pm") => afg::Mod::Pm { rate, dev: get("--dev")?.unwrap_or(90.0), shape },
        Some(x) => return Err(format!("--mod {}: am, fm or pm", x)),
        None => afg::Mod::None,
    };
    Ok(md)
}
