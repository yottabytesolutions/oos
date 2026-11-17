// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use oos_hal::dho::seq::{self, Step};
use oos_hal::dho::{adc, afe};

pub(crate) fn run(a: &[String]) {
    acquire("run", a);
}

pub(super) fn acquire(cmd: &str, a: &[String]) {
    let o = match super::acquisition::parse(a) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("oos-fpgad: {}", e);
            std::process::exit(2);
        }
    };
    let dir = std::env::var("OOS_CAL_DIR").unwrap_or_else(|_| "/rigol/data".into());
    let cal = if o.s.afe { afe::Cal::load(&dir).ok() } else { None };
    let cal_adc = adc::CalAdc::load(&dir).ok();
    let mut state = seq::State::handover(o.app_mask, cal_adc.as_ref());
    let (st, plan_p) = match seq::plan(&mut state, &o.s, cal.as_ref(), cal_adc.as_ref()) {
        Ok((mut st, p)) => {
            if o.no_read {
                st.retain(|x| !matches!(x, Step::Read { .. }));
            }
            (st, p)
        }
        Err(e) => {
            eprintln!("oos-fpgad: {}", e);
            std::process::exit(2);
        }
    };
    #[cfg(not(target_os = "linux"))]
    let _ = plan_p;
    if cmd == "plan" || !o.execute {
        super::plan::print(&st);
        if cmd == "run" {
            eprintln!("oos-fpgad: dry run; add --execute to write the FPGA");
        }
        return;
    }
    #[cfg(target_os = "linux")]
    if let Err(e) = execute(&o, &st, plan_p, state, cal.as_ref(), cal_adc.as_ref()) {
        eprintln!("oos-fpgad: {}", e);
        std::process::exit(1);
    }
    #[cfg(not(target_os = "linux"))]
    eprintln!("oos-fpgad: run needs the unit (Linux)");
}

#[cfg(target_os = "linux")]
fn execute(
    o: &super::Opt,
    st: &[Step],
    plan_p: oos_hal::dho::hz::Params,
    mut state: seq::State,
    cal: Option<&afe::Cal>,
    cal_adc: Option<&adc::CalAdc>,
) -> Result<(), String> {
    use oos_hal::dho::bus::{Bar1, Bus, FPGA_VERSION};
    let mut bar = Bar1::open().map_err(|e| format!("/dev/xdma0_bypass: {}", e))?;
    let v = bar.rd(0x4000);
    if v != FPGA_VERSION {
        return Err(format!("unexpected FPGA version {:#x}, not touching it", v));
    }
    let mut afe_bus = if o.s.afe { Some(afe::GpioAfe::open().map_err(|e| format!("AFE GPIO: {}", e))?) } else { None };
    let mut c2h = std::fs::File::open("/dev/xdma0_c2h_0").map_err(|e| format!("/dev/xdma0_c2h_0: {}", e))?;
    let t0 = std::time::Instant::now();
    let log = |m: String| eprintln!("{:8.1} ms  {}", t0.elapsed().as_secs_f64() * 1e3, m);
    let mut raw = oos_hal::dho::bus::DmaBuf::new();
    let bytes = if o.s.frames > 1 {
        // batch: configuration, then arm / wait for the ring / read the recorded frames (max 30 ms extra)
        let mut state = seq::State::handover(o.app_mask, cal_adc);
        let (cfg, p) = state.configure(&o.s, cal, cal_adc)?;
        seq::execute(&cfg, &mut bar, afe_bus.as_mut(), &mut c2h, log)?;
        let mut dma = oos_hal::dho::bus::DmaBuf::new();
        let n = seq::acquire_batch(&mut state, &o.s, &p, &mut bar, &mut c2h, &mut dma, false, 30, log)?;
        eprintln!("{:8.1} ms  {} frames of {} bytes", t0.elapsed().as_secs_f64() * 1e3, n, seq::frame_bytes(&o.s, &p));
        if o.raw_too {
            seq::read_raw_again(&mut state, &o.s, &p, &mut bar, &mut c2h, &mut raw, n.max(1) - 1, log)?;
        }
        dma.into_vec()
    } else {
        let b = seq::execute(st, &mut bar, afe_bus.as_mut(), &mut c2h, log)?;
        if o.raw_too {
            seq::read_raw_again(&mut state, &o.s, &plan_p, &mut bar, &mut c2h, &mut raw, 0, log)?;
        }
        b
    };
    save_output(o, &bytes, &raw, t0)
}

#[cfg(target_os = "linux")]
fn save_output(
    o: &super::Opt,
    bytes: &[u8],
    raw: &oos_hal::dho::bus::DmaBuf,
    t0: std::time::Instant,
) -> Result<(), String> {
    if o.raw_too {
        let path = format!("{}.raw", o.out);
        std::fs::write(&path, raw.data()).map_err(|e| format!("{}: {}", path, e))?;
        eprintln!(
            "{:8.1} ms  {} raw bytes of the same record -> {}",
            t0.elapsed().as_secs_f64() * 1e3,
            raw.data().len(),
            path
        );
    }
    std::fs::write(&o.out, bytes).map_err(|e| format!("{}: {}", o.out, e))?;
    eprintln!("{:8.1} ms  {} bytes -> {}", t0.elapsed().as_secs_f64() * 1e3, bytes.len(), o.out);
    Ok(())
}
