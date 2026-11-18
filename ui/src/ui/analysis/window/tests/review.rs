// SPDX-License-Identifier: GPL-2.0-or-later
//! Review regressions for stale display and history lifetime.

use super::*;

#[test]
fn settings_changes_mark_the_accepted_result_stale_immediately() {
    for change in 0..4 {
        let view = if change == 0 { AView::Eye } else { AView::Spectrum };
        let mut ui = fixture(view, 10_000, 2e-7, |k| if k % 86 < 43 { -1.0 } else { 1.0 });
        ui.measure_in_background();
        calculate(&mut ui);
        assert!(!updating(&ui));
        let old = run(prepare(&ui, view, false).unwrap(), &mut None);
        match change {
            0 => ui.m.eye_src = WaveId::Ch(1),
            1 => ui.m.spec_src = WaveId::Ch(1),
            2 => ui.m.spec_center = 1e6,
            _ => ui.m.spec_span = 1e6,
        }
        assert!(updating(&ui), "settings change {change} must mark a fresh result immediately");
        assert!(!accept(&mut ui, old));
        assert!(updating(&ui), "dropping stale completion must retain the marker");
        calculate(&mut ui);
        assert!(!updating(&ui));
        ui.stop_workers(std::time::Instant::now() + crate::ui::lifecycle::test_wait::HANG_GUARD);
    }
}

#[test]
fn span_label_follows_the_model_and_current_record_before_completion() {
    let mut ui = fixture(AView::Spectrum, 10_000, 1e-7, |k| (k as f32).sin());
    calculate(&mut ui);
    ui.m.spec_center = 2e6;
    ui.m.spec_span = 1e6;
    assert_eq!(ui.value(Item::SpecSpan), "1 MHz");
    ui.m.spec_center = 0.0;
    ui.m.spec_span = 1e9;
    assert_eq!(ui.value(Item::SpecSpan), "5 MHz");
    let rec = ui.last[0].as_mut().unwrap();
    Arc::make_mut(&mut rec.raw.as_mut().unwrap().frame).dt = 2e-7;
    assert_eq!(ui.value(Item::SpecSpan), "2.5 MHz");
}

#[test]
fn unrelated_channel_changes_and_single_arms_preserve_spectrum_history() {
    for trace in [1, 2] {
        for single in [false, true] {
            let mut ui = fixture(AView::Spectrum, 10_000, 1e-7, |k| (k as f32 * 0.1).sin());
            ui.m.spec_trace = trace;
            ui.m.spec_wf = true;
            change::reset(&mut ui);
            calculate(&mut ui);
            ui.ana_activate(Item::SpecLimitSet);
            let mut history = None;
            let done = run(prepare(&ui, AView::Spectrum, false).unwrap(), &mut history);
            assert!(accept(&mut ui, done));
            if !single {
                ui.m.ch[2].offset += 1.0;
            }
            advance_record(&mut ui, single);
            let done = run(prepare(&ui, AView::Spectrum, false).unwrap(), &mut history);
            assert!(accept(&mut ui, done));
            assert_eq!(ui.azr.spec_avg.as_ref().unwrap().count(), 2, "trace {trace}, SINGLE {single}");
            assert_eq!(ui.azr.wf.as_ref().unwrap().rows.len(), 2);
            assert_eq!(ui.azr.limit_n, 2);
            ui.m.ch[0].invert = !ui.m.ch[0].invert;
            let done = run(prepare(&ui, AView::Spectrum, false).unwrap(), &mut history);
            assert!(accept(&mut ui, done));
            assert_eq!(ui.azr.spec_avg.as_ref().unwrap().count(), 1);
            assert_eq!(ui.azr.limit_n, 1);
        }
    }
}

#[test]
fn eye_hits_pool_across_single_stamps_but_clear_restarts_them() {
    let mut ui = fixture(AView::Eye, 10_000, 2e-7, |k| if k % 86 < 43 { -1.0 } else { 1.0 });
    change::reset(&mut ui);
    let mut history = None;
    let done = run(prepare(&ui, AView::Eye, false).unwrap(), &mut history);
    assert!(accept(&mut ui, done));
    let hits = ui.azr.eye.as_ref().unwrap().hits.clone();
    advance_record(&mut ui, true);
    let done = run(prepare(&ui, AView::Eye, false).unwrap(), &mut history);
    assert!(accept(&mut ui, done));
    assert_eq!(ui.azr.eye.as_ref().unwrap().hits, hits.iter().map(|n| n * 2).collect::<Vec<_>>());
    ui.ana_activate(Item::EyeClear);
    change::begin_step(&mut ui, 0.04).unwrap();
    let done = run(prepare(&ui, AView::Eye, false).unwrap(), &mut history);
    assert!(accept(&mut ui, done));
    assert_eq!(ui.azr.eye.as_ref().unwrap().hits, hits);
}

#[test]
fn disconnected_analysis_worker_falls_back_to_synchronous_results() {
    let mut ui = fixture(AView::Spectrum, 10_000, 1e-7, |k| (k as f32 * 0.1).sin());
    let expected = run(prepare(&ui, AView::Spectrum, false).unwrap(), &mut None).state.spec;
    ui.measure_in_background();
    // A disconnected sender pins the failed-send case without racing the real worker's lifetime.
    let (tx, rx) = std::sync::mpsc::channel();
    drop(rx);
    let worker = ui.meas_bg.as_mut().unwrap();
    let original = std::mem::replace(&mut worker.tx, tx);
    drop(original);
    // Retain the join handle so this test accounts for the retired worker too.
    let join = std::mem::replace(&mut worker.join, std::thread::spawn(|| {}));
    join.join().unwrap();
    ui.analyzer_update();
    assert!(ui.meas_bg.is_none());
    assert_eq!(ui.azr.spec, expected);
    assert!(!ui.azr.work.pending);
    assert!(ui.azr.work.samples() > 0);
    assert!(ui.azr.work.accepted_key.as_ref() == Some(&key(&ui)));
}

mod equivalence;
mod verification;

#[test]
fn lifecycle_reset_rejects_pre_reset_analyzer_results_at_the_same_pair() {
    let mut ui = fixture(AView::Eye, 10_000, 2e-7, |k| if k % 86 < 43 { -1.0 } else { 1.0 });
    ui.m.run = RunState::Stop;
    let before = key(&ui).generation;
    let record = ui.last[0].clone();
    let done = run(prepare(&ui, AView::Eye, false).unwrap(), &mut None);
    // A direct reset pins the revision independently of acquisition identity or configuration changes.
    crate::ui::lifecycle::reset(&mut ui);
    ui.last[0] = record;
    assert_eq!(key(&ui).generation.0, before.0);
    assert_eq!(key(&ui).generation.1, before.1 + 1);
    assert!(!accept(&mut ui, done));
    assert!(ui.azr.eye.is_none());
}
