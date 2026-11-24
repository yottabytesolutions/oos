// SPDX-License-Identifier: GPL-2.0-or-later

use super::{status, Io};
use crate::{
    board::{CounterWant, Gen, HwShared},
    perfguard::deep::acq_lock as lock,
};
use oos_hal::dho::{adc, afe, afg};
use std::time::{Duration, Instant};

pub(super) struct Service {
    generator_cal: Option<afg::Cal>,
    generator_io: Option<afg::Io>,
    generator_done: Option<Gen>,
    xadc_at: Option<Instant>,
    counting: Option<(CounterWant, Instant)>,
    count_n: u64,
}

impl Service {
    pub(super) fn new(cal_dir: &str) -> Self {
        Self {
            generator_cal: afg::Cal::load(cal_dir),
            generator_io: None,
            generator_done: None,
            xadc_at: None,
            counting: None,
            count_n: 0,
        }
    }

    pub(super) fn before_pass(out: &HwShared) -> Option<Option<CounterWant>> {
        let g = lock(out);
        (!g.record_flow.shutdown).then_some(g.counter_want)
    }

    pub(super) fn readings(&mut self, io: &mut Io, out: &HwShared, counter: Option<CounterWant>) {
        super::super::units::counter_tick(&mut io.bar, out, counter, &mut self.counting, &mut self.count_n);
        if self.xadc_at.is_none_or(|t| t.elapsed() >= Duration::from_secs(1)) {
            self.xadc_at = Some(Instant::now());
            let xadc = oos_hal::dho::board::read_xadc(&mut io.bar);
            let probe = oos_hal::model::platform().caps.la.then(|| oos_hal::dho::la::probe_present().ok());
            let mut g = lock(out);
            g.xadc = xadc;
            if let Some(probe) = probe {
                g.la_probe = probe;
            }
        }
    }

    pub(super) fn calibrate(
        &mut self,
        io: &mut Io,
        cal_dir: &str,
        out: &HwShared,
        (chans, zero, adc_cores): (Vec<usize>, bool, bool),
    ) -> bool {
        let mut reloaded = false;
        let result = match (io.afe_bus.as_mut(), io.cal.as_ref()) {
            (Some(afe), Some(cal)) => {
                let cancel = || lock(out).cal_cancel;
                let mut unit = oos_hal::dho::selfcal::Unit {
                    bar: &mut io.bar,
                    afe,
                    c2h: &mut io.c2h,
                    state: &mut io.state,
                    cal,
                    cal_adc: io.cal_adc.as_ref(),
                    cancel: &cancel,
                    adc_new: None,
                };
                let adc_base = if adc_cores { io.cal_adc.as_ref() } else { None };
                oos_hal::dho::selfcal::run_all_with(
                    &mut unit,
                    cal,
                    adc_base,
                    &chans,
                    zero,
                    &mut |done, total, stage| {
                        lock(out).cal_progress = Some((done, total, stage.to_string()));
                    },
                )
                .and_then(|report| {
                    let saved =
                        super::super::selfcal::selfcal_saved(oos_hal::dho::selfcal::save(&report, cal_dir), || {
                            reload_calibration(io, cal_dir);
                            reloaded = true;
                        })?;
                    Ok(format!(
                        "{} configurations, largest change {:.2} %, {}",
                        report.measured,
                        report.worst * 100.0,
                        saved
                    ))
                })
            }
            _ => Err("self-cal needs the AFE bus (--afe) and the calibration files".into()),
        };
        let message = match &result {
            Ok(saved) => format!("done, {}", saved),
            Err(e) => format!("failed: {}", e),
        };
        status(out, format!("self-cal CH{:?}: {}", chans.iter().map(|c| c + 1).collect::<Vec<_>>(), message));
        let mut g = lock(out);
        (g.cal_progress, g.cal_result, g.cal_cancel) = (None, Some(result), false);
        reloaded
    }

    pub(super) fn generator_on(&self) -> bool {
        self.generator_done.is_some_and(|done| done.0)
    }

    pub(super) fn generator_changed(&self, want: Option<Gen>) -> bool {
        want != self.generator_done
    }

    pub(super) fn generator(&mut self, io: &mut Io, out: &HwShared, want: Option<Gen>) {
        // Models without a generator leave its GPIO lines and FPGA block untouched.
        if let Some((on, f, a, o, wave, md)) = want.filter(|_| oos_hal::model::platform().caps.has_gen()) {
            if self.generator_io.is_none() {
                self.generator_io = afg::Io::open().map_err(|e| status(out, format!("generator GPIO: {}", e))).ok();
            }
            if let Some(generator) = self.generator_io.as_mut() {
                let cal = self.generator_cal.as_ref();
                let quick = self
                    .generator_done
                    .filter(|old| on && old.0 && old.4 == wave && old.5 == md && md == afg::Mod::None)
                    .and_then(|old| afg::retune(cal, wave, old.1 * 1e6, f * 1e6, a * 1e9, o * 1e9, generator.relays()));
                let ops = if let Some(quick) = quick {
                    quick
                } else if on {
                    afg::start_mod(cal, wave, f * 1e6, a * 1e9, o * 1e9, generator.relays(), md)
                } else {
                    afg::idle(a * 1e9, o * 1e9, f * 1e6, generator.relays())
                };
                generator.apply(&mut io.bar, &ops);
                let state = if on { format!("{:?} {} Hz {} Vpp {} V {:?}", wave, f, a, o, md) } else { "off".into() };
                status(out, format!("generator: {}", state));
            }
        }
        self.generator_done = want;
    }
}

fn reload_calibration(io: &mut Io, cal_dir: &str) {
    io.cal = afe::Cal::load(cal_dir).ok();
    io.cal_adc = adc::CalAdc::load(cal_dir).ok();
    if let Some(cal) = io.cal_adc.as_mut() {
        io.trim_overlay.apply(cal);
    }
}
