// SPDX-License-Identifier: GPL-2.0-or-later
//! Compare worker dispatch with DSP on the raw slice and deterministic offline analysis.

use super::*;
use oos_dsp::{power, spectrum};

#[test]
fn spectrum_worker_equals_direct_raw_slice_dsp_and_synchronous_path() {
    let dt = 1e-7;
    let mut ui = fixture(AView::Spectrum, 100_000, dt, |k| (TAU * 123_000.0 * k as f64 * dt).sin() as f32);
    let raw = ui.last[0].as_mut().unwrap().raw.as_mut().unwrap();
    raw.i0 = 1000;
    raw.i1 = 91_000;
    let job = prepare(&ui, AView::Spectrum, false).unwrap();
    let raw = ui.last[0].as_ref().unwrap().raw.as_ref().unwrap();
    let settings = spectrum::Settings {
        center: ui.m.spec_center,
        span: ui.m.spec_span,
        rbw: spectrum::enbw(ui.m.spec_win) / ((raw.i1 - raw.i0) as f64 * dt),
        window: ui.m.spec_win,
        points: job.context.points,
        ref_level_dbv: ui.m.spec_ref as f64,
        ..Default::default()
    };
    let direct = spectrum::analyze(&raw.frame.ch[0][raw.i0..raw.i1], dt, &settings, &mut None);
    let worker = run(job, &mut None);
    assert_eq!(worker.state.spec.as_ref().unwrap(), &direct);
    assert!(ui.meas_bg.is_none());
    ui.analyzer_update();
    assert_eq!(ui.azr.spec, worker.state.spec);
}

fn quality_lines(q: power::Quality) -> Vec<String> {
    vec![
        format!("Vrms {}   Irms {}", si(q.vrms, "V"), si(q.irms, "A")),
        format!("P {}   S {}   Q {}", si(q.p_real, "W"), si(q.s_apparent, "VA"), si(q.q_reactive, "var")),
        format!("PF {:.3}   phase {:.1}\u{b0}   f {}", q.pf, q.phase_deg, si(q.freq, "Hz")),
        format!("crest V {:.2}   crest I {:.2}   {} cycles", q.crest_v, q.crest_i, q.cycles),
    ]
}

#[test]
fn power_worker_equals_direct_raw_slice_quality_and_synchronous_path() {
    let dt = 1e-6;
    let mut ui = fixture(AView::Power, 100_000, dt, |k| (TAU * 50.0 * k as f64 * dt).sin() as f32);
    let raw = ui.last[0].as_mut().unwrap().raw.as_mut().unwrap();
    raw.i0 = 1000;
    raw.i1 = 91_000;
    Arc::make_mut(&mut raw.frame).ch[1] =
        (0..100_000).map(|k| (0.5 * (TAU * 50.0 * k as f64 * dt - 0.3).sin()) as f32).collect();
    let direct = power::quality(&raw.frame.ch[0][raw.i0..raw.i1], &raw.frame.ch[1][raw.i0..raw.i1], dt, None);
    let worker = run(prepare(&ui, AView::Power, false).unwrap(), &mut None);
    assert_eq!(worker.state.power, quality_lines(direct));
    assert!(ui.meas_bg.is_none());
    ui.analyzer_update();
    assert_eq!(ui.azr.power, worker.state.power);
}

#[test]
fn envelope_pair_and_hires_windows_match_measurement_threshold_and_frequency() {
    for mode in 0..3 {
        let mut ui = fixture(AView::Eye, 10_000, 2e-7, |k| {
            let level = (TAU * k as f64 / 100.0).sin() as f32;
            level + if k % 2 == 0 { -0.01 } else { 0.01 }
        });
        let rec = ui.last[0].as_mut().unwrap();
        rec.pairs = true;
        Arc::make_mut(&mut rec.raw.as_mut().unwrap().frame).envelope = true;
        if mode == 1 {
            rec.raw = None;
        } else if mode == 2 {
            ui.m.acq = AcqMode::HiRes;
        }
        let record = ui.last[0].clone().unwrap();
        ui.measure_rec(&record, usize::MAX);
        let mut job = prepare(&ui, AView::Eye, false).unwrap();
        let (v, dt) = job.context.meas_wave(&job.record, 0);
        assert_eq!((v.as_slice(), dt), ui.meas_slice(&record, 0));
        let st = stats(&v, dt);
        assert_eq!(0.5 * (st.top + st.base), 0.5 * (ui.stats[0].top + ui.stats[0].base));
        let f1 = oos_dsp::measure::analyze_min(&v, dt, 0.4 * ui.m.ch[0].volts_per_div() as f64).get(Meas::Freq);
        assert!(f1.is_finite());
        assert_eq!(f1, ui.ana[0].get(Meas::Freq));
    }
}
