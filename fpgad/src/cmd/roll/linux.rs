// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use oos_hal::dho::bus::{Bar1, Bus};
use oos_hal::dho::{adc, afe, scu, seq};
use std::io::Write as _;

type Configure<'a> = &'a dyn Fn(&mut scu::Shadows) -> Vec<(u32, u32)>;

pub(super) fn execute(
    o: &super::super::Opt,
    secs: f64,
    scan: bool,
    slow_cfg: impl Fn(&mut scu::Shadows) -> Vec<(u32, u32)>,
) -> Result<(), String> {
    let name = if scan { "scan" } else { "roll" };
    use oos_hal::dho::bus::FPGA_VERSION;
    let dir = std::env::var("OOS_CAL_DIR").unwrap_or_else(|_| "/rigol/data".into());
    let cal = if o.s.afe { afe::Cal::load(&dir).ok() } else { None };
    let cal_adc = adc::CalAdc::load(&dir).ok();
    let mut bar = Bar1::open().map_err(|e| format!("/dev/xdma0_bypass: {}", e))?;
    if bar.rd(0x4000) != FPGA_VERSION {
        return Err("unexpected FPGA version, not touching it".into());
    }
    let mut afe_bus = if o.s.afe { Some(afe::GpioAfe::open().map_err(|e| format!("AFE GPIO: {}", e))?) } else { None };
    let mut c2h = std::fs::File::open("/dev/xdma0_c2h_0").map_err(|e| format!("/dev/xdma0_c2h_0: {}", e))?;
    let mut state = seq::State::handover(o.app_mask, cal_adc.as_ref());
    let (cfg, _) = state.configure(&o.s, cal.as_ref(), cal_adc.as_ref())?;
    seq::execute(&cfg, &mut bar, afe_bus.as_mut(), &mut c2h, |_| {})?;
    bar.apply(&slow_cfg(&mut state.sh));
    bar.apply(&oos_hal::dho::roll::start(&mut state.sh, o.s.mask));
    let out = std::fs::File::create(&o.out).map_err(|e| format!("{}: {}", o.out, e))?;
    let mut stream = Stream {
        out,
        scan,
        mask: o.s.mask,
        t0: std::time::Instant::now(),
        total: 0,
        reads: 0,
        empty: 0,
        tsec: 0,
        csec: 0,
        last: oos_hal::dho::roll::ScanInfo::default(),
        sweeps: 0,
        sweep_chunks: 0,
    };
    stream.run(&mut bar, &mut c2h, &mut state, &slow_cfg, secs)?;
    bar.apply(&oos_hal::dho::roll::stop(&mut state.sh));
    println!(
        "{}: {} chunks in {} reads ({} empty polls) in {:.1} s; 0x1208 = {:#010x}",
        name,
        stream.total,
        stream.reads,
        stream.empty,
        stream.t0.elapsed().as_secs_f64(),
        bar.rd(0x1208)
    );
    if scan {
        println!("scan: {} complete sweeps", stream.sweeps);
    }
    Ok(())
}

struct Stream {
    out: std::fs::File,
    scan: bool,
    mask: u32,
    t0: std::time::Instant,
    total: usize,
    reads: usize,
    empty: usize,
    tsec: u64,
    csec: usize,
    last: oos_hal::dho::roll::ScanInfo,
    sweeps: usize,
    sweep_chunks: usize,
}

impl Stream {
    fn run(
        &mut self,
        bar: &mut Bar1,
        c2h: &mut std::fs::File,
        state: &mut seq::State,
        slow_cfg: Configure<'_>,
        secs: f64,
    ) -> Result<(), String> {
        while self.t0.elapsed().as_secs_f64() < secs {
            bar.apply(&oos_hal::dho::roll::fill_pulse(&state.sh));
            let n = bar.rd(0x11e8);
            let c = oos_hal::dho::roll::chunks(n);
            if c > 0 {
                self.read(c2h, c, n)?;
            }
            if self.scan && self.advance_scan(bar, state, slow_cfg) {
                continue;
            }
            if c == 0 {
                self.empty += 1;
                std::thread::sleep(std::time::Duration::from_millis(9));
                continue;
            }
            if self.t0.elapsed().as_secs() > self.tsec {
                println!(
                    "{:5.1} s  {} chunks in the last second (last 0x11e8 = {}, {} chunks)",
                    self.t0.elapsed().as_secs_f64(),
                    self.csec,
                    n,
                    c
                );
                self.tsec = self.t0.elapsed().as_secs();
                self.csec = 0;
            }
        }
        Ok(())
    }

    fn read(&mut self, c2h: &mut std::fs::File, c: usize, n: u32) -> Result<(), String> {
        let buf = oos_hal::dho::bus::read_dma(c2h, c * oos_hal::dho::roll::CHUNK_BYTES, |_, _| {})
            .map_err(|e| format!("c2h read of {} chunks (0x11e8 = {}): {}", c, n, e))?;
        self.out.write_all(&buf).map_err(|e| e.to_string())?;
        self.total += c;
        self.csec += c;
        self.sweep_chunks += c;
        self.reads += 1;
        Ok(())
    }

    fn advance_scan(&mut self, bar: &mut Bar1, state: &mut seq::State, slow_cfg: Configure<'_>) -> bool {
        // after every read, also an empty one
        let r = oos_hal::dho::roll::SCAN_INFO_REGS.map(|x| bar.rd(x));
        let info = oos_hal::dho::roll::scan_info(r[0], r[1]);
        if info != self.last {
            println!(
                "{:5.2} s  0x1204 = {:#010x}, 0x40d0 = {:#x} after {} chunks of this sweep",
                self.t0.elapsed().as_secs_f64(),
                r[0],
                r[1],
                self.sweep_chunks
            );
            self.last = info;
        }
        if info.done {
            // stock state 8 (RUN): stop, clear the average, 500 ms, configure again, next sweep
            self.sweeps += 1;
            bar.apply(&oos_hal::dho::roll::scan_done(&mut state.sh));
            std::thread::sleep(std::time::Duration::from_millis(500));
            bar.apply(&slow_cfg(&mut state.sh));
            bar.apply(&oos_hal::dho::roll::start(&mut state.sh, self.mask));
            self.sweep_chunks = 0;
            return true;
        }
        false
    }
}
