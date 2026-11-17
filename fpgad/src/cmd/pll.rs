// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::options;
#[cfg(target_os = "linux")]
use super::options::num;

#[cfg(target_os = "linux")]
pub(super) fn run(a: &[String]) -> Result<(), String> {
    use oos_hal::dho::bus::{Bar1, Bus};
    let has = |f: &str| a.iter().any(|x| x == f);
    use oos_hal::dho::boot::{regs, PllPins};
    let mut pll = oos_hal::dho::boot::io::GpioPll::open().map_err(|e| format!("PLL lines: {}", e))?;
    let send = |pll: &mut oos_hal::dho::boot::io::GpioPll, w: [u8; 3]| {
        for (p, v) in regs::pll_edges(w) {
            pll.set(p, v);
        }
        std::thread::sleep(std::time::Duration::from_micros(regs::PLL_WORD_GAP_US));
    };
    // idle state of the stock module after insmod: LE high (the first word then starts with a falling edge)
    pll.set(regs::PllPin::Le, true);
    std::thread::sleep(std::time::Duration::from_millis(10));
    if let Some(v) = options::value(a, "--r0").flatten() {
        // diagnostic: one R0 word only (e.g. 0x221d = POWERDOWN set: the counter must drop to 0)
        let v = num(v)?;
        send(&mut pll, [0, (v >> 8) as u8, v as u8]);
        println!("LMX2582: R0 <- {:#06x}", v);
        let mut bar = Bar1::open().map_err(|e| format!("/dev/xdma0_bypass: {}", e))?;
        for _ in 0..3 {
            std::thread::sleep(std::time::Duration::from_millis(300));
            println!("0x1210 = {}", bar.rd(0x1210));
        }
        return Ok(());
    }
    if has("--reset") {
        send(&mut pll, [0, 0x00, 0x02]);
        send(&mut pll, [0, 0x00, 0x00]);
        println!("LMX2582 reset (R0 = 0x0002, 0x0000)");
    }
    // Rigol's table, or the experimental one of limits.conf `adc.clock_hz` (as the cold boot)
    let table = oos_hal::dho::boot::pll_table()?;
    if oos_hal::dho::clock::overclocked() {
        println!("LMX2582: EXPERIMENTAL ADC clock {} Hz from limits.conf", oos_hal::dho::clock::adc_hz());
    }
    for w in table {
        send(&mut pll, w);
    }
    println!("LMX2582: {} words written", table.len());
    std::thread::sleep(std::time::Duration::from_millis(100));
    let mut bar = Bar1::open().map_err(|e| format!("/dev/xdma0_bypass: {}", e))?;
    for _ in 0..3 {
        println!("0x1210 = {}", bar.rd(0x1210));
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    Ok(())
}

/// `pll --freq <Hz> --dry`: the table `oos_hal::dho::lmx2582::table_for` makes and its decode, nothing written. Without
/// --dry this command refuses other clocks; unit results are in docs/models.md 4.1. None = use the stock table.
pub(super) fn dry(a: &[String]) -> Result<Option<String>, String> {
    use oos_hal::dho::lmx2582;
    let freq = options::value(a, "--freq")
        .map(|v| v.ok_or("--freq <Hz>")?.parse::<f64>().map_err(|e| format!("--freq: {}", e)))
        .transpose()?;
    if !a.iter().any(|x| x == "--dry") {
        return match freq {
            Some(f) if f != 1.25e9 => Err(format!(
                "pll --freq {}: this command writes only 1.25 GHz; use --dry for other clocks (unit results: docs/models.md 4.1)",
                f
            )),
            _ => Ok(None),
        };
    }
    let f = freq.unwrap_or(1.25e9);
    let t = lmx2582::table_for(f)?;
    let d = lmx2582::Fields::decode(&t)?;
    let mut s = format!(
        "LMX2582 table for {} Hz (dry run, nothing written; R0 last starts the VCO calibration)\n{}\n",
        f,
        lmx2582::format_table(&t)
    );
    for l in d.lines() {
        s += &format!("  {}\n", l);
    }
    let changed: Vec<String> = t
        .iter()
        .zip(oos_hal::dho::boot::regs::PLL_TABLE.iter())
        .filter(|(x, y)| x != y)
        .map(|(x, _)| format!("R{}", x[0]))
        .collect();
    s += &format!("  changed vs. 1.25 GHz: {}\n", if changed.is_empty() { "none".into() } else { changed.join(" ") });
    for n in d.notes() {
        s += &format!("  note: {}\n", n);
    }
    Ok(Some(s))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pll_dry_run() {
        let args = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        let s = dry(&args(&["--freq", "1.5e9", "--dry"])).unwrap().unwrap();
        assert!(s.contains("R38=002c") && s.contains("R45=0002") && s.contains("4500.000000 MHz"), "{}", s);
        assert!(dry(&args(&["--freq", "1500000000"])).unwrap_err().contains("only 1.25 GHz"));
        assert_eq!(dry(&args(&["--freq", "1.25e9"])).unwrap(), None);
        assert_eq!(dry(&args(&["--reset"])).unwrap(), None);
        assert!(dry(&args(&["--freq", "5e9", "--dry"])).is_err());
    }
}
