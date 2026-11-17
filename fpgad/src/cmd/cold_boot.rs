// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::fpga_boot::BOOTED_FILE;
use super::options::{self, num};
use crate::pcie::{down as pcie_down, up as pcie_up};
use oos_hal::bootstatus;
use oos_hal::dho::afe;

pub(super) fn run(a: &[String]) -> Result<(), String> {
    let has = |f: &str| a.iter().any(|x| x == f);
    let dir = std::env::var("OOS_CAL_DIR").unwrap_or_else(|_| "/rigol/data".into());
    // cold-boot --execute
    let progress = options::value(a, "--progress").flatten().map(str::to_owned);
    let say = |state: &str, stage: &str, detail: &str| {
        if let Some(p) = &progress {
            bootstatus::append(p, state, stage, detail);
        }
    };
    let cal = oos_hal::dho::boot::CalFiles::load(&dir);
    check_cal(&cal, &dir, &say);
    let opt = oos_hal::dho::boot::Options { ignore_clock: has("--ignore-clock"), ..Default::default() };
    // the start-up screen's stage of the running step: ok when the next one starts
    let mut cur: Option<&'static str> = None;
    let report = if has("--power-on") { power_on(a, &cal, &opt, &say, &mut cur)? } else { cold_boot(&cal, &opt)? };
    print!("{}", oos_hal::dho::boot::format_log(&report.log));
    match report.result {
        Ok(o) => {
            println!("cold boot ok: {}", o.summary());
            if let Some(c) = cur {
                say("ok", c, "");
            }
            if !o.warnings.is_empty() {
                say("warn", "Cold-boot checks", &o.warnings.join("; "));
            }
            if !o.idelay_scanned {
                say("warn", oos_hal::dho::boot::stage(12), "eye scan failed, stored lane delays loaded");
            }
            if !o.trim_done {
                say("warn", oos_hal::dho::boot::stage(14), "ADC trim timeout");
            }
            check_temps(&mut bar_for_temps(), &say);
        }
        Err(e) => {
            say("fail", cur.unwrap_or("FPGA image"), &e);
            return Err(format!("cold boot stopped: {}", e));
        }
    }
    Ok(())
}

/// Start-up self-test: every calibration file OOS reads is there with a good CRC (a missing one only warns: the
/// scope runs on defaults, less accurate).
fn check_cal(cal: &oos_hal::dho::boot::CalFiles, dir: &str, say: &dyn Fn(&str, &str, &str)) {
    const STAGE: &str = "Calibration data";
    say("run", STAGE, "");
    let mut missing = Vec::new();
    if cal.adc.is_none() {
        missing.push("cal_adc.hex");
    }
    if cal.ddr.is_none() {
        missing.push("cal_ddr.hex");
    }
    if afe::Cal::load_from(dir).is_err() {
        missing.push("vertical");
    }
    // only units with a generator have cal_afg.hex (the DHO800 and the DHO900 without "S" never had one)
    if oos_hal::model::platform().caps.has_gen() && oos_hal::dho::afg::Cal::load(dir).is_none() {
        missing.push("cal_afg.hex");
    }
    if missing.is_empty() {
        say("ok", STAGE, "all files read, CRC ok");
    } else {
        say("warn", STAGE, &format!("missing or damaged: {}", missing.join(", ")));
    }
}

fn bar_for_temps() -> Option<oos_hal::dho::bus::Bar1> {
    oos_hal::dho::bus::Bar1::open().ok()
}

/// Start-up self-test: SoC and FPGA temperatures below the guard's limits (`board::is_hot`).
fn check_temps(bar: &mut Option<oos_hal::dho::bus::Bar1>, say: &dyn Fn(&str, &str, &str)) {
    use oos_hal::dho::board;
    const STAGE: &str = "Temperatures";
    say("run", STAGE, "");
    let zones = board::thermal_zones(std::path::Path::new("/sys/class/thermal"));
    let fpga = bar.as_mut().and_then(|b| board::read_xadc(b)).map(|x| x.0);
    let soc = zones.iter().map(|z| z.temp).fold(f32::NAN, f32::max);
    // 0x1438 reads 0 on the DHO924S, under stock as well: no reading is no fault
    let txt = format!("SoC {:.0} C{}", soc, fpga.map_or(String::new(), |t| format!(", FPGA {:.0} C", t)));
    if board::is_hot(&zones, fpga) {
        say("warn", STAGE, &txt);
    } else {
        say("ok", STAGE, &txt);
    }
}

fn power_on(
    a: &[String],
    cal: &oos_hal::dho::boot::CalFiles,
    opt: &oos_hal::dho::boot::Options,
    say: &dyn Fn(&str, &str, &str),
    cur: &mut Option<&'static str>,
) -> Result<oos_hal::dho::boot::Report, String> {
    let has = |f: &str| a.iter().any(|x| x == f);
    let slot = options::value(a, "--slot").flatten().map(num).transpose()?.unwrap_or(0x40_0000);
    // --booted without the file (the early BOOT command failed): send it here
    let booted = has("--booted")
        .then(|| std::fs::read_to_string(BOOTED_FILE).ok().and_then(|t| t.trim().parse::<f64>().ok()))
        .flatten();
    if booted.is_none() {
        say("run", "FPGA image", "loading the image from flash");
    }
    // after a repeated BOOT command the stage keeps the warning's text, not "PCIe link"
    let repeats = std::cell::Cell::new(0u32);
    let up = || {
        if repeats.get() == 0 {
            say("run", "FPGA image", "PCIe link");
        }
        pcie_up()
    };
    let noted = |n: u32| {
        repeats.set(n);
        say(
            "warn",
            "FPGA image",
            &format!(
                "the BOOT command got no answer, sent again ({} of {})",
                n,
                oos_hal::dho::boot::io::FPGA_BOOT_RETRIES
            ),
        );
    };
    let mut step = |n: u32| {
        let g = oos_hal::dho::boot::stage(n);
        if *cur != Some(g) {
            if let Some(c) = *cur {
                say("ok", c, "");
            }
            say("run", g, "");
            *cur = Some(g);
        }
    };
    let r = oos_hal::dho::boot::io::power_on(slot, booted, up, pcie_down, noted, cal, opt, &mut step);
    if let Err(e) = &r {
        // before the first step: the BOOT command / PCIe, or the PLL / AFE lines (e.g. EBUSY)
        let stage = if e.starts_with("PLL lines") || e.starts_with("AFE lines") { "GPIO lines" } else { "FPGA image" };
        say("fail", cur.unwrap_or(stage), e);
    }
    r
}

fn cold_boot(
    cal: &oos_hal::dho::boot::CalFiles,
    opt: &oos_hal::dho::boot::Options,
) -> Result<oos_hal::dho::boot::Report, String> {
    use oos_hal::dho::bus::Bar1;
    let mut bar = Bar1::open().map_err(|e| format!("/dev/xdma0_bypass: {}", e))?;
    let mut pll = oos_hal::dho::boot::io::GpioPll::open().map_err(|e| format!("PLL lines: {}", e))?;
    let mut afe = oos_hal::dho::afe::GpioAfe::open().map_err(|e| format!("AFE lines: {}", e))?;
    let mut clk = oos_hal::dho::boot::SysClock::default();
    Ok(oos_hal::dho::boot::cold_boot(
        oos_hal::dho::boot::Hw {
            bus: &mut bar,
            clock: &mut clk,
            pll: Some(&mut pll),
            afe: Some(&mut afe),
            early: None,
        },
        cal,
        opt,
    ))
}
