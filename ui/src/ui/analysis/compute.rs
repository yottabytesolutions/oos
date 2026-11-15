// SPDX-License-Identifier: GPL-2.0-or-later
//! Full-data analyser DSP, run on the existing measurement worker.

use super::window::Context;
use super::*;

impl Context {
    pub(in crate::ui) fn meas_wave(&mut self, rec: &Record, c: usize) -> (Vec<f32>, f64) {
        if let Some(r) = rec.raw.as_ref().filter(|_| self.m.acq != AcqMode::HiRes) {
            if r.i1 <= r.frame.ch[c].len() && r.i1 - r.i0 > rec.ch[c].len() {
                self.azr.work.copied(r.i1 - r.i0);
                return (r.frame.ch[c][r.i0..r.i1].to_vec(), r.frame.dt);
            }
        }
        self.azr.work.copied(rec.ch[c].len());
        (rec.ch[c].clone(), rec.dt)
    }
}

impl Context {
    pub(super) fn calculate(&mut self, rec: &Record, view: AView, pane: bool) {
        match view {
            AView::Spectrum => self.spectrum_update(rec, pane),
            AView::Eye => self.eye_update(rec),
            AView::Power => self.power_update(rec),
            AView::Step => self.bw_update(rec),
            AView::Scope | AView::Bode => {}
        }
    }

    /// The spectrum of the source from the record: the view, or `pane` beside the waveform.
    fn spectrum_update(&mut self, rec: &Record, pane: bool) {
        let c = self.m.spec_src;
        if rec.ch[c].is_empty() {
            self.azr.spec = None;
            self.azr.peaks.clear();
            return;
        }
        let (v, dt) = self.meas_wave(rec, c);
        let f1 = oos_dsp::measure::analyze_min(&v, dt, 0.4 * self.m.ch[c].volts_per_div() as f64).get(Meas::Freq);
        let (mut center, mut span) = (self.m.spec_center, self.m.spec_span);
        if self.follow {
            // no frequency this record: the span stays
            self.azr.spec_stop =
                spectrum::auto_stop(f1, PANE_HARMS[self.m.spec_harm.min(PANE_HARMS.len() - 1)], self.azr.spec_stop);
            (center, span) = (self.azr.spec_stop / 2.0, self.azr.spec_stop);
        }
        // Auto RBW: the finest the record allows (window ENBW / record time)
        let finest = spectrum::enbw(self.m.spec_win) / (v.len().max(1) as f64 * dt);
        let s = spectrum::Settings {
            center,
            span,
            rbw: if self.m.spec_rbw > 0.0 { self.m.spec_rbw } else { finest },
            window: self.m.spec_win,
            averaging: 16,
            trace: [
                spectrum::Trace::Normal,
                spectrum::Trace::Average,
                spectrum::Trace::MaxHold,
                spectrum::Trace::MinHold,
            ][self.m.spec_trace.min(3)],
            detector: spectrum::Detector::Peak,
            points: self.points,
            ref_level_dbv: self.m.spec_ref as f64,
        };
        let sp = spectrum::analyze(&v, dt, &s, &mut self.azr.spec_avg);
        self.azr.peaks = spectrum::rank_lines(&sp, Some(f1), panes::PEAKS_MAX);
        if self.m.spec_sort == 0 {
            self.azr.peaks.sort_by(|a, b| a.f.total_cmp(&b.f));
        } else {
            self.azr.peaks.sort_by(|a, b| b.dbv.total_cmp(&a.dbv));
        }
        if self.m.spec_wf && !pane {
            self.azr.wf.get_or_insert_with(|| spectrum::Waterfall::new(120)).push(&sp);
        }
        self.azr.dist = if self.m.spec_dist && !pane { spectrum::distortion(&v, dt, None, 10) } else { None };
        if let Some(l) = &self.azr.limit {
            self.azr.limit_n += 1;
            if let Some(w) = spectrum::limit_check(&sp, l) {
                self.azr.limit_over += 1;
                if self.azr.limit_worst.is_none_or(|x| w.1 > x.1) {
                    self.azr.limit_worst = Some(w);
                }
            }
        }
        self.azr.spec = Some(sp);
    }

    fn power_update(&mut self, rec: &Record) {
        let (cv, ci) = (self.m.pw_v, self.m.pw_i);
        if rec.ch[cv].is_empty() || (rec.ch[ci].is_empty() && self.m.pw_mode != PowerMode::Ripple) {
            self.azr.power = vec!["Voltage / current channel off: Switch on source in the menu".into()];
            return;
        }
        let (v, dt) = self.meas_wave(rec, cv);
        let i = if rec.ch[ci].is_empty() { Vec::new() } else { self.meas_wave(rec, ci).0 };
        self.azr.power = match self.m.pw_mode {
            PowerMode::Quality => {
                let q = power::quality(&v, &i, dt, None);
                vec![
                    format!("Vrms {}   Irms {}", si(q.vrms, "V"), si(q.irms, "A")),
                    format!("P {}   S {}   Q {}", si(q.p_real, "W"), si(q.s_apparent, "VA"), si(q.q_reactive, "var")),
                    format!("PF {:.3}   phase {:.1}\u{b0}   f {}", q.pf, q.phase_deg, si(q.freq, "Hz")),
                    format!("crest V {:.2}   crest I {:.2}   {} cycles", q.crest_v, q.crest_i, q.cycles),
                ]
            }
            PowerMode::Harmonics => {
                let h = power::harmonics(&i, dt, None, 15);
                let mut out = vec!["Current harmonics vs. IEC 61000-3-2 class A".to_string()];
                out.extend(h.iter().map(|(n, a, lim, pass)| {
                    format!(
                        "H{:<2} {:>10}  limit {:>10}  {}",
                        n,
                        si(*a, "A"),
                        if lim.is_finite() { si(*lim, "A") } else { "-".into() },
                        if *pass { "pass" } else { "FAIL" }
                    )
                }));
                out
            }
            PowerMode::Switching => {
                let s = power::switching(&v, &i, dt, 10.0);
                vec![
                    format!("E on {}   E off {}", si(s.e_on, "J"), si(s.e_off, "J")),
                    format!("E cond {}   P avg {}", si(s.e_cond, "J"), si(s.p_avg, "W")),
                    format!("{} transitions", s.transitions.len()),
                ]
            }
            PowerMode::Ripple => {
                let (pp, ac) = power::ripple(&v);
                let (pk, tpk, i2t) = if i.is_empty() { (f64::NAN, f64::NAN, f64::NAN) } else { power::inrush(&i, dt) };
                vec![
                    format!("Ripple {} pp   {} rms AC", si(pp, "V"), si(ac, "V")),
                    format!("Inrush peak {} at {}   I\u{b2}t {}", si(pk, "A"), si(tpk, "s"), si(i2t, "A\u{b2}s")),
                ]
            }
        };
    }
}
