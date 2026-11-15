// SPDX-License-Identifier: GPL-2.0-or-later
//! Full-record correctness and asynchronous handoff regressions.

use super::*;
use std::f64::consts::TAU;
use std::sync::Arc;

fn fixture(view: AView, n: usize, dt: f64, signal: impl Fn(usize) -> f32) -> Ui {
    let mut ui = Ui::for_tests(Source::Demo { t: 0.0 });
    ui.m.view = view;
    ui.m.spec_harm = 0;
    ui.m.eye_rate = 115200.0;
    ui.m.pw_i = 1;
    let v: Vec<f32> = (0..n).map(signal).collect();
    let mut frame = crate::board::Frame { dt, stamp: ui.m.hw_stamp, seq: 37, ..Default::default() };
    frame.ch[0] = v.clone();
    frame.ch[1] = v;
    let ch = std::array::from_fn(|c| frame.ch[c].iter().step_by(n.div_ceil(3600)).copied().collect());
    ui.last[0] = Some(Record {
        ch,
        dt: dt * n.div_ceil(3600) as f64,
        t0: 0.0,
        triggered: true,
        partial: false,
        raw: Some(crate::wave::Raw { frame: Arc::new(frame), i0: 0, i1: n, t0: 0.0 }),
        la: None,
        pairs: false,
    });
    ui
}

fn calculate(ui: &mut Ui) {
    let done = run(prepare(ui, ui.m.view, false).unwrap(), &mut None);
    assert!(accept(ui, done));
}

#[test]
fn deep_spectrum_keeps_one_mhz_and_real_nyquist() {
    let dt = 8e-9;
    let mut ui = fixture(AView::Spectrum, 2_500_000, dt, |k| (TAU * 1e6 * k as f64 * dt).sin() as f32);
    calculate(&mut ui);
    let sp = ui.azr.spec.as_ref().unwrap();
    assert!((sp.freq(sp.dbv.len() - 1) - 62.5e6).abs() < 1.0);
    let peak = ui.strongest_line().unwrap();
    assert!((peak.f - 1e6).abs() < 20_000.0, "{peak:?}");
    assert!(peak.dbv > -4.0, "{peak:?}");
    ui.m.spec_span = 1e9;
    calculate(&mut ui);
    assert_eq!(ui.ana_value(Item::SpecSpan).unwrap(), "62.5 MHz");
}

#[test]
fn deep_uart_eye_at_one_ms_has_edges() {
    let dt = 8e-9;
    // UART 0x55: start low, alternating data bits, stop high, repeated without gaps.
    let mut ui = fixture(AView::Eye, 1_250_000, dt, |k| {
        let bit = (k as f64 * dt * 115200.0) as usize % 10;
        if bit.is_multiple_of(2) {
            -1.0
        } else {
            1.0
        }
    });
    ui.m.tscale = tscales().iter().position(|&v| v == 0.001).unwrap();
    calculate(&mut ui);
    let (_, _, period) = ui.azr.eye_res.as_ref().expect("full UART record supplies edges");
    assert!((period * 115200.0 - 1.0).abs() < 0.001);
    assert!(!ui.azr.page.img.is_empty());
    assert!(!ui.azr.page.status[1].contains("Under 3 edges"));
}

#[test]
fn deep_power_keeps_whole_line_cycles_and_early_inrush() {
    let dt = 40e-9;
    let mut ui = fixture(AView::Power, 2_500_000, dt, |k| (TAU * 50.0 * k as f64 * dt).sin() as f32);
    calculate(&mut ui);
    assert!(ui.azr.power[2].contains("50.0 Hz"), "{:?}", ui.azr.power);
    assert!(ui.azr.power[3].contains("5 cycles"), "{:?}", ui.azr.power);
    let r = ui.last[0].as_mut().unwrap().raw.as_mut().unwrap();
    Arc::make_mut(&mut r.frame).ch[1][1000] = 10.0;
    ui.m.pw_mode = PowerMode::Ripple;
    calculate(&mut ui);
    assert!(ui.azr.power[1].contains("10.0 A"), "{:?}", ui.azr.power);
}

#[test]
fn bandwidth_preserves_full_frame_and_five_thousand_bins() {
    let dt = 8e-9;
    let mut ui =
        fixture(AView::Step, 2_500_000, dt, |k| ((TAU * 1_000_013.0 * k as f64 * dt).sin() * 10.0).tanh() as f32);
    ui.m.bw_bins = 5000;
    // Even when the measurement window is small, equivalent-time folding uses the full frame.
    ui.last[0].as_mut().unwrap().raw.as_mut().unwrap().i1 = 4000;
    calculate(&mut ui);
    assert!(ui.azr.bw.is_some(), "{}", ui.azr.bw_info);
    assert!(ui.azr.bw_info.contains("125 MSa/s"), "{}", ui.azr.bw_info);
    assert!(ui.azr.bw_info.contains("2.50M"), "{}", ui.azr.bw_info);
}

#[test]
fn stale_results_are_dropped_after_each_invalidation() {
    for change in 0..6 {
        let mut ui = fixture(AView::Eye, 100_000, 2e-7, |k| if k % 86 < 43 { -1.0 } else { 1.0 });
        let done = run(prepare(&ui, AView::Eye, false).unwrap(), &mut None);
        match change {
            0 => ui.m.hw_stamp += 1,
            1 => ui.meas_n += 1,
            2 => {
                ui.ana_activate(Item::EyeClear);
            }
            3 => ui.m.eye_src = 1,
            4 => ui.area.h -= 10,
            _ => ui.m.acq = AcqMode::HiRes,
        }
        assert!(!accept(&mut ui, done), "invalidation {change}");
        assert!(ui.azr.eye.is_none());
    }
}

#[test]
fn background_completion_is_displayed_after_stop_without_main_dsp() {
    let mut ui = fixture(AView::Eye, 100_000, 2e-7, |k| if k % 86 < 43 { -1.0 } else { 1.0 });
    ui.measure_in_background();
    ui.analyzer_update();
    assert!(ui.azr.eye.is_none());
    let done = ui.meas_bg.as_ref().unwrap().analysis_rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
    ui.m.run = RunState::Stop;
    assert!(accept(&mut ui, done));
    assert!(ui.azr.eye_res.is_some());
    assert_eq!(ui.azr.work.samples(), 0);
    ui.stop_workers(std::time::Instant::now() + std::time::Duration::from_secs(2));
}

#[test]
fn busy_updates_coalesce_to_the_newest_record_and_preserve_record_identity() {
    let mut ui = fixture(AView::Spectrum, 1000, 1e-7, |k| (k as f32).sin());
    ui.measure_in_background();
    ui.meas_bg.as_mut().unwrap().analysis_busy = true;
    for seq in 38..42 {
        let r = ui.last[0].as_mut().unwrap().raw.as_mut().unwrap();
        Arc::make_mut(&mut r.frame).seq = seq;
        ui.analyzer_update();
        assert!(ui.azr.work.pending);
        assert_eq!(ui.azr.work.samples(), 0);
    }
    ui.meas_bg.as_mut().unwrap().analysis_busy = false;
    poll(&mut ui);
    assert!(!ui.azr.work.pending);
    let done = ui.meas_bg.as_ref().unwrap().analysis_rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
    assert_eq!(done.record, Some((41, ui.m.hw_stamp)));
    assert!(accept(&mut ui, done));
    ui.now += 0.6;
    assert!(updating(&ui), "an older finished result must indicate lag");
    ui.stop_workers(std::time::Instant::now() + std::time::Duration::from_secs(2));
}

#[test]
fn a_raw_record_of_an_old_acquisition_generation_is_not_submitted() {
    let mut ui = fixture(AView::Spectrum, 1000, 1e-7, |k| (k as f32).sin());
    ui.measure_in_background();
    ui.m.hw_stamp += 1;
    ui.analyzer_update();
    assert!(!ui.meas_bg.as_ref().unwrap().analysis_busy);
    assert!(ui.azr.work.pending);
    ui.stop_workers(std::time::Instant::now() + std::time::Duration::from_secs(2));
}

mod review;

#[test]
fn running_analyzer_source_step_snaps_before_submitting_without_broad_resets() {
    for (view, item) in [(AView::Eye, Item::EyeSrc), (AView::Spectrum, Item::SpecSrc)] {
        let mut ui = fixture(view, 1024, 1e-6, |k| (k as f32 * 0.1).sin());
        ui.m.run = RunState::Run;
        ui.m.ch[0].on = true;
        ui.m.ch[1].on = false;
        ui.m.meas.clear();
        ui.m.meas2.clear();
        ui.m.spec_src = 0;
        ui.m.eye_src = 0;
        change::reset(&mut ui);
        ui.dirty_map = false;
        ui.mstat = vec![MStat { n: 7, ..Default::default() }];
        ui.ph.map[0][3] = 19;
        let picture = ui.ph.map.clone();
        ui.adjust(item, 1);
        let submitted = ui.azr.work.accepted_key.as_ref().expect("intake submits synchronous job");
        assert_eq!((submitted.eye.0, submitted.spectrum.0), (0, 0), "job must use an enabled source");
        assert_eq!((ui.m.eye_src, ui.m.spec_src), (0, 0));
        assert_eq!(ui.mstat[0].n, 7);
        assert!(!ui.dirty_map);
        change::begin_step(&mut ui, 0.04).unwrap();
        assert_eq!(ui.mstat[0].n, 7);
        assert!(!ui.dirty_map);
        assert_eq!(ui.ph.map, picture);
        assert_eq!(change::counters(&ui).4, 0);
    }
}
