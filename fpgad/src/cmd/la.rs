// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

pub(crate) fn run(a: &[String]) -> Result<(), String> {
    use oos_hal::dho::la;
    let exec = a.iter().any(|x| x == "--execute");
    let dir = std::env::var("OOS_CAL_DIR").unwrap_or_else(|_| "/rigol/data".into());
    if !matches!(a.first().map(String::as_str), Some("status" | "thr")) {
        return Err("afg sine|off, la status|thr".into());
    }
    let cal = la::Cal::load(&dir).unwrap_or_default();
    let w = if a[0] == "thr" {
        let v = |i: usize| a.get(i).ok_or("la thr <pod1 V> <pod2 V>")?.parse::<f64>().map_err(|e| e.to_string());
        Some(la::thresholds(&cal, v(1)?, v(2)?))
    } else {
        None
    };
    println!(
        "la: zero codes {:?}{}",
        cal.zero,
        w.map_or(String::new(), |w| format!(", write {:#06x} <- {:#010x}", w.0, w.1))
    );
    #[cfg(target_os = "linux")]
    {
        use oos_hal::dho::bus::{Bar1, Bus};
        let mut bar = Bar1::open().map_err(|e| format!("/dev/xdma0_bypass: {}", e))?;
        if let (Some(w), true) = (w, exec) {
            bar.wr(w.0, w.1);
        }
        let l = la::lines(bar.rd(la::LINES));
        println!(
            "probe {}, thresholds {:#010x}, lines D15..D0 {}",
            match la::probe_present() {
                Ok(true) => "present",
                Ok(false) => "absent",
                Err(_) => "unknown",
            },
            bar.rd(la::THRESHOLD),
            l.iter().rev().map(|&b| if b { '1' } else { '0' }).collect::<String>()
        );
    }
    if w.is_some() && !exec {
        eprintln!("oos-fpgad: dry run; add --execute");
    }
    Ok(())
}
