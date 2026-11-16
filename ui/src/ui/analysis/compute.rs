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
            AView::Power => self.power_update(rec),
            AView::Step => self.bw_update(rec),
            AView::Eye => self.eye_update(rec),
            AView::Scope | AView::Bode | AView::Magnetic | AView::Inverter | AView::Dpt => {}
        }
    }

    /// The spectrum of the source from the record: the view, or `pane` beside the waveform.
    fn spectrum_update(&mut self, rec: &Record, pane: bool) {
        let Some(c) = self.m.spec_src.ch().filter(|&c| c < NCH) else { return };
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
        let (Some(cv), Some(ci)) = (self.m.pw_v.ch().filter(|&c| c < NCH), self.m.pw_i.ch().filter(|&c| c < NCH))
        else {
            return;
        };
        if rec.ch[cv].is_empty() || (rec.ch[ci].is_empty() && self.m.pw_mode != PowerMode::Ripple) {
            self.azr.power = vec!["Voltage / current channel off: Switch on source in the menu".into()];
            return;
        }
        let (v, dt) = self.meas_wave(rec, cv);
        let i = if rec.ch[ci].is_empty() { Vec::new() } else { self.meas_wave(rec, ci).0 };
        self.azr.power = power_rows(&self.m, &v, &i, dt);
    }
}

/// The rows of the Power page for voltage `v` and current `i` (empty when off) at sample interval `dt`.
fn power_rows(m: &Model, v: &[f32], i: &[f32], dt: f64) -> Vec<String> {
    match m.pw_mode {
        PowerMode::Quality => {
            let q = power::quality(v, i, dt, None);
            vec![
                format!("Vrms {}   Irms {}", si(q.vrms, "V"), si(q.irms, "A")),
                format!("P {}   S {}   Q {}", si(q.p_real, "W"), si(q.s_apparent, "VA"), si(q.q_reactive, "var")),
                format!("PF {:.3}   phase {:.1}\u{b0}   f {}", q.pf, q.phase_deg, si(q.freq, "Hz")),
                format!("crest V {:.2}   crest I {:.2}   {} cycles", q.crest_v, q.crest_i, q.cycles),
            ]
        }
        PowerMode::Harmonics => harmonic_rows(i, dt),
        PowerMode::Switching => {
            let s = power::switching(v, i, dt, 10.0);
            vec![
                format!("E on {}   E off {}", si(s.e_on, "J"), si(s.e_off, "J")),
                format!("E cond {}   P avg {}", si(s.e_cond, "J"), si(s.p_avg, "W")),
                format!("dv/dt {} / {}", si(s.dvdt_rise, "V/s"), si(s.dvdt_fall, "V/s")),
                format!("di/dt {} / {}", si(s.didt_rise, "A/s"), si(s.didt_fall, "A/s")),
                format!("RDS(on) {}", si(s.rds_on, "\u{3a9}")),
                format!("{} transitions", s.transitions.len()),
            ]
        }
        PowerMode::Ripple => ripple_rows(m, v, i, dt),
        PowerMode::Capacitance => {
            vec![format!("C {}", si(power::input_capacitance(v, i, dt), "F")), "The record is the turn-on event".into()]
        }
        PowerMode::Rail => {
            let r = power::rail(v, dt, m.pw_ref, None, None);
            let refn = m.pw_ref.map(|v| si(v, "V")).unwrap_or_else(|| "top".into());
            vec![
                format!("Overshoot {}   undershoot {}", si(r.overshoot, "V"), si(r.undershoot, "V")),
                format!("DC {}   turn-on max {}", si(r.dc, "V"), si(r.turn_on, "V")),
                format!("Reference {}", refn),
            ]
        }
    }
}

fn harmonic_rows(i: &[f32], dt: f64) -> Vec<String> {
    let h = power::harmonics(i, dt, None, 15);
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

/// Ripple, its line and switching tones (set frequencies, else measured) and the inrush of the current.
fn ripple_rows(m: &Model, v: &[f32], i: &[f32], dt: f64) -> Vec<String> {
    let (pp, ac) = power::ripple(v);
    let (pk, tpk, i2t) = if i.is_empty() { (f64::NAN, f64::NAN, f64::NAN) } else { power::inrush(i, dt) };
    let fl = if m.pw_fline > 0.0 { m.pw_fline } else { power::line_hz(v, dt) };
    let fs = if m.pw_fsw > 0.0 { m.pw_fsw } else { power::switch_hz(v, dt) };
    let line = power::tone_ripple(v, dt, fl);
    let sw = power::switching_ripple(v, dt, fs);
    vec![
        format!("Ripple {} pp   {} rms AC", si(pp, "V"), si(ac, "V")),
        format!("Line {}  {} pp  {} rms", si(fl, "Hz"), si(line.pp, "V"), si(line.rms, "V")),
        format!("Switch {}  {} pp  {} rms", si(fs, "Hz"), si(sw.pp, "V"), si(sw.rms, "V")),
        format!("Inrush peak {} at {}   I\u{b2}t {}", si(pk, "A"), si(tpk, "s"), si(i2t, "A\u{b2}s")),
    ]
}
