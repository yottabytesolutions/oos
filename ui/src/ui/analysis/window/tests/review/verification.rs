// SPDX-License-Identifier: GPL-2.0-or-later
//! Verification regressions for Bandwidth capture identity and Spectrum levels.

use super::*;

fn bandwidth_capture_change(change: usize) {
    let dt = 8e-9;
    let mut ui = fixture(AView::Step, 20_000, dt, |k| ((TAU * 1_012_345.0 * k as f64 * dt).sin() * 10.0).tanh() as f32);
    ui.now = 10.0;
    let mut history = None;
    let done = run(prepare(&ui, AView::Step, false).unwrap(), &mut history);
    assert!(accept(&mut ui, done));
    let old = ui.azr.bw.clone();
    let old_info = ui.azr.bw_info.clone();
    assert!(old.is_some(), "{old_info}");
    let raw = ui.last[0].as_mut().unwrap().raw.as_mut().unwrap();
    let frame = Arc::make_mut(&mut raw.frame);
    frame.seq += 1;
    frame.dt *= 2.0;
    frame.ch[0].iter_mut().for_each(|v| *v *= 2.0);
    // Ordinary frames in the same acquisition still obey the half-second throttle.
    ui.now += 0.05;
    let done = run(prepare(&ui, AView::Step, false).unwrap(), &mut history);
    assert!(done.state.bw == old, "same acquisition must retain the throttled result");
    assert_eq!(done.state.bw_info, old_info);
    match change {
        0 => {
            // SINGLE arms a new acquisition stamp without changing analyser settings.
            advance_record(&mut ui, true);
        }
        1 => ui.m.tscale += 1,
        2 => ui.m.delay += 0.001,
        _ => ui.m.mdepth += 1,
    }
    ui.now += 0.05;
    let expected = run(prepare(&ui, AView::Step, false).unwrap(), &mut None);
    assert!(expected.state.bw.is_some(), "{}", expected.state.bw_info);
    assert!(expected.state.bw != old, "fixture must produce a different step response");
    assert_ne!(expected.state.bw_info, old_info);
    let done = run(prepare(&ui, AView::Step, false).unwrap(), &mut history);
    ui.m.run = RunState::Stop;
    assert!(accept(&mut ui, done), "completion after STOP must be accepted");
    assert!(ui.azr.bw == expected.state.bw, "new capture must bypass the old throttle");
    assert_eq!(ui.azr.bw_info, expected.state.bw_info);
}

#[test]
fn bandwidth_single_within_half_a_second_computes_the_new_record() {
    bandwidth_capture_change(0);
}

#[test]
fn bandwidth_time_scale_change_bypasses_the_throttle() {
    bandwidth_capture_change(1);
}

#[test]
fn bandwidth_delay_change_bypasses_the_throttle() {
    bandwidth_capture_change(2);
}

#[test]
fn bandwidth_memory_depth_change_bypasses_the_throttle() {
    bandwidth_capture_change(3);
}

fn spectrum_level_change(item: Item) {
    let mut ui = fixture(AView::Spectrum, 10_000, 1e-7, |k| (k as f32 * 0.1).sin());
    ui.m.spec_trace = 1;
    ui.m.spec_wf = true;
    ui.measure_in_background();
    let mut history = None;
    for _ in 0..2 {
        let done = run(prepare(&ui, AView::Spectrum, false).unwrap(), &mut history);
        assert!(accept(&mut ui, done));
    }
    assert_eq!(ui.azr.spec_avg.as_ref().unwrap().count(), 2);
    assert_eq!(ui.azr.wf.as_ref().unwrap().rows.len(), 2);
    assert!(!updating(&ui));
    let old = run(prepare(&ui, AView::Spectrum, false).unwrap(), &mut history);
    // Keep the real worker idle while retaining a stale in-flight completion.
    ui.meas_bg.as_mut().unwrap().analysis_busy = true;
    assert!(ui.ana_adjust(item, 1));
    assert!(updating(&ui), "changed level must mark the accepted result stale");
    assert!(!accept(&mut ui, old), "old-level completion must be rejected");
    assert!(updating(&ui));
    let done = run(prepare(&ui, AView::Spectrum, false).unwrap(), &mut history);
    assert!(accept(&mut ui, done));
    assert!(!updating(&ui));
    assert_eq!(ui.azr.spec_avg.as_ref().unwrap().count(), 1);
    assert_eq!(ui.azr.wf.as_ref().unwrap().rows.len(), 1);
    assert_eq!(ui.azr.spec.as_ref().unwrap().ref_level_dbv, ui.m.spec_ref as f64);
    ui.stop_workers(std::time::Instant::now() + crate::ui::lifecycle::test_wait::HANG_GUARD);
}

#[test]
fn spectrum_reference_change_rejects_stale_results_and_restarts_history() {
    spectrum_level_change(Item::SpecRef);
}

#[test]
fn spectrum_margin_change_rejects_stale_results_and_restarts_history() {
    spectrum_level_change(Item::SpecMargin);
}
