// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

#[cfg(target_os = "linux")]
use super::options;

#[cfg(target_os = "linux")]
pub(super) fn run(a: &[String]) -> Result<(), String> {
    use oos_hal::dho::bus::{Bar1, Bus};
    let has = |f: &str| a.iter().any(|x| x == f);
    if ui_running() {
        return Err("oos-ui is running: stop it first (it owns the ADC and the measure unit)".into());
    }
    match a.first().map(String::as_str) {
        Some("slots") => {
            let runs = options::value(a, "--runs")
                .flatten()
                .map(|x| x.parse::<u32>().map_err(|e| e.to_string()))
                .transpose()?
                .unwrap_or(4)
                .max(1);
            let mut bar = Bar1::open().map_err(|e| format!("/dev/xdma0_bypass: {}", e))?;
            let mut got = Vec::new();
            for _ in 0..runs {
                got.push(
                    oos_hal::dho::levels::run(&mut bar, &mut |us| {
                        std::thread::sleep(std::time::Duration::from_micros(us as u64))
                    })
                    .ok_or("measure unit did not finish")?,
                );
            }
            for l in slot_lines(&got) {
                println!("{}", l);
            }
        }
        Some("offset") => {
            let usage = "adc offset <core 0..7> <steps -1024..1023> [--execute]";
            let core = a.get(1).ok_or(usage)?.parse::<usize>().map_err(|e| e.to_string())?;
            let o = a.get(2).ok_or(usage)?.parse::<i32>().map_err(|e| e.to_string())?;
            let w = oos_hal::dho::adc::Adc::core_offset_word(core, o)?;
            if !has("--execute") {
                println!(
                    "dry run: would write 0x1034 <- {:#010x} (core {} offset {} steps); add --execute",
                    w, core, o
                );
                return Ok(());
            }
            let mut bar = Bar1::open().map_err(|e| format!("/dev/xdma0_bypass: {}", e))?;
            bar.wr(0x1034, w);
            std::thread::sleep(std::time::Duration::from_micros(100));
            println!("0x1034 <- {:#010x} (core {} offset {} steps)", w, core, o);
        }
        _ => return Err("adc slots [--runs n] | adc offset <core> <steps> [--execute]".into()),
    }
    Ok(())
}

/// An oos-ui process is running (/proc/<pid>/comm).
#[cfg(target_os = "linux")]
fn ui_running() -> bool {
    std::fs::read_dir("/proc")
        .map(|d| {
            d.flatten().any(|e| std::fs::read_to_string(e.path().join("comm")).is_ok_and(|c| c.trim() == "oos-ui"))
        })
        .unwrap_or(true)
}

/// `adc slots`: per slot the mean (16-bit scale), lowest min/highest max (12-bit), offset in ADC steps (16 codes).
fn slot_lines(runs: &[[oos_hal::dho::levels::Slot; 8]]) -> Vec<String> {
    let n = runs.len().max(1) as f64;
    let mean: [f64; 8] = std::array::from_fn(|k| runs.iter().map(|r| r[k].mean).sum::<f64>() / n);
    let all = mean.iter().sum::<f64>() / 8.0;
    (0..8)
        .map(|k| {
            let lo = runs.iter().map(|r| r[k].min).min().unwrap_or(0);
            let hi = runs.iter().map(|r| r[k].max).max().unwrap_or(0);
            format!("slot {}: mean {:.1}  min {}  max {}  {:+.2} steps", k, mean[k], lo, hi, (mean[k] - all) / 16.0)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_lines_show_each_slot_against_the_average() {
        use oos_hal::dho::levels::Slot;
        let mut r = [Slot { min: 2040, max: 2056, mean: 32768.0 }; 8];
        r[5].mean = 32768.0 + 7.0 * 16.0;
        let l = slot_lines(&[r, r]);
        assert!(l[5].ends_with("+6.12 steps"), "{}", l[5]);
        assert!(l[0].ends_with("-0.88 steps") && l[0].contains("min 2040  max 2056"), "{}", l[0]);
        let mut bad = std::collections::HashMap::from([(0x1210u32, 0u32)]);
        assert!(super::super::status::status_lines(&mut |o| *bad.entry(o).or_insert(0))[3].contains("NOT locked"));
    }
}
