// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

mod browse;

fn segmented_setup() -> (Ui, crate::board::HwShared) {
    segmented_setup_of(50)
}

fn segmented_setup_of(segments: usize) -> (Ui, crate::board::HwShared) {
    let shared = crate::board::HwShared::default();
    let mut ui = Ui::for_tests(Source::Hw { shared: shared.clone(), seen: 0 });
    ui.m.run = RunState::Run;
    ui.m.zoom = false;
    ui.m.ch[0].on = false;
    ui.m.ch[1].on = true;
    ui.m.ch[2].on = true;
    ui.m.trig_src = 1;
    ui.m.trig_mode = TrigMode::Normal;
    ui.m.tscale = tscales().iter().position(|&v| v == 200e-9).unwrap();
    ui.relayout();
    change::reset(&mut ui);
    ui.step(0.04);
    // The unit finding enabled segments after a setup change during Normal RUN.
    ui.m.seg_hw = true;
    ui.m.segments = segments;
    ui.settings_changed();
    ui.step(0.04);
    (ui, shared)
}

#[test]
fn segmented_single_after_setup_during_normal_run_records_fifty() {
    use crate::board::scripted::SegmentWorker;
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let (mut ui, shared) = segmented_setup();
    assert_eq!(ui.m.run, RunState::Run);
    let before = lock(&shared).seg.run;
    let mut remote = crate::scpi::Scpi::open(0u16, false);
    remote.exec_line(&mut ui, &Canvas::new(1, 1), ":SINGle");
    ui.step(0.04);
    assert!(lock(&shared).seg.run > before);
    let mut worker = SegmentWorker::new();
    worker.tick(&shared);
    assert_eq!(lock(&shared).seg.phase, crate::board::SegPhase::Recording);
    for _ in 0..100 {
        worker.tick(&shared);
        ui.step(0.04);
        if ui.m.run == RunState::Stop {
            break;
        }
    }
    assert_eq!((ui.m.run, ui.records, ui.seg_n), (RunState::Stop, 50, 50));
    assert_eq!(event("acq_seg_start="), 1);
    assert_eq!(change::counters(&ui).4, 0);
}

#[test]
fn segmented_first_run_can_start_with_command_zero() {
    use crate::board::scripted::SegmentWorker;
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let (_, shared) = segmented_setup();
    lock(&shared).seg.run = 0;
    let mut worker = SegmentWorker::new();
    worker.tick(&shared);
    assert_eq!(lock(&shared).seg.phase, crate::board::SegPhase::Recording);
    assert_eq!(event("acq_seg_start="), 1);
}

#[test]
fn segmented_statistics_change_keeps_the_recording_pair() {
    use crate::board::scripted::SegmentWorker;
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let (mut ui, shared) = segmented_setup();
    let mut worker = SegmentWorker::new();
    worker.tick(&shared);
    let old = change::acquisition::identity(&ui);
    let run = lock(&shared).seg.run;
    ui.m.meas.push((oos_dsp::measure::Meas::Vpp, crate::model::WaveId::Ch(1)));
    ui.settings_changed();
    ui.step(0.04);
    let id = change::acquisition::identity(&ui);
    assert_eq!((id.gen(), id.epoch()), (old.gen(), old.epoch()));
    assert_ne!(id.stats_gen(), old.stats_gen());
    assert_eq!(lock(&shared).seg.run, run);
    worker.tick(&shared);
    assert_eq!(lock(&shared).seg.phase, crate::board::SegPhase::Reading);
    assert_eq!(event("acq_seg_start="), 1);
}

#[test]
fn segmented_pair_change_during_dma_rejects_frames_times_and_browse() {
    use crate::board::scripted::SegmentWorker;
    for browse in [false, true] {
        let _scope = crate::drawpipe::test_scope::Scope::new();
        let (mut ui, shared) = segmented_setup();
        let mut worker = SegmentWorker::new();
        worker.tick(&shared);
        worker.tick(&shared);
        if browse {
            worker.tick(&shared);
            worker.tick(&shared);
        }
        let old = change::acquisition::identity(&ui);
        let run = lock(&shared).seg.run;
        ui.key(key::CLEAR);
        ui.step(0.04);
        let id = change::acquisition::identity(&ui);
        {
            let mut g = lock(&shared);
            g.acquisition = crate::board::identity::Publication::new(old, false);
            g.seg.run = run;
            g.seg.times.clear();
            g.seg.read = 0;
            g.seg.want = browse.then_some(0);
        }
        worker.change_pair_during_read(&shared, id);
        worker.tick(&shared);
        let g = lock(&shared);
        assert_eq!(g.acquisition.identity(), id);
        assert!(g.queue.is_empty() && g.seg.times.is_empty() && g.seg.shown.is_none());
        assert_eq!(g.seg.read, 0);
        drop(g);
        assert!(crate::drawpipe::counters().losses.worker_push.load(std::sync::atomic::Ordering::Relaxed) > 0);
        if browse {
            // Done survives a pair boundary; RUN restarts it when the UI's new run arrives.
            lock(&shared).seg.run = ui.seg_run;
        }
        worker.tick(&shared);
        assert_eq!(lock(&shared).seg.phase, crate::board::SegPhase::Recording);
        assert_eq!(event("acq_seg_start="), 2);
    }
}

#[test]
fn segmented_single_while_running_replaces_the_previous_done_phase() {
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let (mut ui, shared) = segmented_setup();
    let old_run = lock(&shared).seg.run;
    {
        let mut g = lock(&shared);
        g.seg.done_run = old_run;
        g.seg.phase = crate::board::SegPhase::Done;
    }
    let mut remote = crate::scpi::Scpi::open(0u16, false);
    remote.exec_line(&mut ui, &Canvas::new(1, 1), ":SINGle");
    ui.step(0.04);
    assert!(lock(&shared).seg.run > old_run, "SINGLE must start a new segmented run even during RUN");
    assert!(!change::proposals::stopping(&ui), "the old Done phase cannot finish the new SINGLE");
    assert!(lock(&shared).seg.running);
    ui.step(0.04);
    assert_eq!(ui.m.run, RunState::Run);
}

#[test]
fn segmented_setup_change_during_run_cannot_finish_from_old_done_state() {
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let (mut ui, shared) = segmented_setup();
    let old_run = lock(&shared).seg.run;
    {
        let mut g = lock(&shared);
        g.seg.done_run = old_run;
        g.seg.phase = crate::board::SegPhase::Done;
    }
    ui.m.mask_src = 1;
    ui.settings_changed();
    ui.step(0.04);
    assert!(lock(&shared).seg.run > old_run, "a new pair needs fresh segment status");
    assert!(!change::proposals::stopping(&ui));
    assert_eq!(ui.seg_view.unwrap().phase, crate::board::SegPhase::Idle);
    ui.step(0.04);
    assert_eq!(ui.m.run, RunState::Run);
}

#[test]
fn segmented_single_restarts_each_pair_during_recording_and_reading() {
    use crate::board::SegPhase;
    for phase in [SegPhase::Recording, SegPhase::Reading] {
        for boundary in ["clear", "gen", "configure"] {
            check_segment_boundary(phase, boundary);
        }
    }
}

fn check_segment_boundary(phase: crate::board::SegPhase, boundary: &str) {
    use crate::board::scripted::SegmentWorker;
    use crate::board::SegPhase;
    let _scope = crate::drawpipe::test_scope::Scope::new();
    // more segments than one read-back batch takes (`seg_read_plan`: half the queue), so Reading spans steps
    let n = crate::board::QUEUE_MAX;
    let (mut ui, shared) = segmented_setup_of(n);
    ui.m.run = RunState::Stop;
    ui.step(0.04);
    ui.key(key::SINGLE);
    ui.step(0.04);
    let old = change::acquisition::identity(&ui);
    let mut worker = SegmentWorker::new();
    worker.tick(&shared);
    assert_eq!(lock(&shared).seg.phase, SegPhase::Recording);
    if phase == SegPhase::Reading {
        // the recording ends, then the first read-back batch
        for _ in 0..50 {
            worker.tick(&shared);
            if lock(&shared).seg.read > 0 {
                break;
            }
        }
        ui.step(0.04);
        assert_eq!(lock(&shared).seg.phase, phase);
        assert!(ui.seg_n > 0);
        let mut remote = crate::scpi::Scpi::open(0u16, false);
        let reply = remote.exec_line(&mut ui, &Canvas::new(1, 1), ":ACQuire:SEGMents:TIMes?");
        assert_eq!(String::from_utf8(reply).unwrap().split(',').count(), lock(&shared).seg.read as usize);
    }
    match boundary {
        "clear" => ui.key(key::CLEAR),
        "gen" => ui.m.mask_src = 1,
        "configure" => ui.m.tscale += 1,
        _ => unreachable!(),
    }
    ui.settings_changed();
    ui.step(0.04);
    let id = change::acquisition::identity(&ui);
    assert_ne!(id, old);
    let records = ui.records;
    worker.tick(&shared);
    assert_eq!(lock(&shared).seg.phase, SegPhase::Recording, "{phase:?} {boundary}");
    assert!(lock(&shared).seg.times.is_empty());
    assert_eq!(event("acq_seg_start="), 2);
    assert_eq!(event("acq_configure="), if boundary == "configure" { 2 } else { 1 });
    for _ in 0..120 {
        worker.tick(&shared);
        let g = lock(&shared);
        assert!(g.queue.iter().all(|f| (f.cfg, f.stamp) == (id.gen(), id.epoch())));
        drop(g);
        ui.step(0.04);
        if ui.m.run == RunState::Stop {
            break;
        }
    }
    assert_eq!(ui.m.run, RunState::Stop, "{phase:?} {boundary}");
    assert_eq!((ui.records - records, ui.seg_n as usize, ui.seg_times.len()), (n as u64, n, n));
    assert_eq!(change::counters(&ui).4, 0);
}

#[test]
fn segmented_worker_restarts_pair_without_a_new_ui_run() {
    use crate::board::scripted::SegmentWorker;
    use crate::board::SegPhase;
    for reading in [false, true] {
        for clear in [false, true] {
            let _scope = crate::drawpipe::test_scope::Scope::new();
            let (mut ui, shared) = segmented_setup();
            let mut worker = SegmentWorker::new();
            worker.tick(&shared);
            if reading {
                worker.tick(&shared);
            }
            let run = lock(&shared).seg.run;
            if clear {
                ui.key(key::CLEAR);
            } else {
                ui.m.mask_src = 1;
                ui.settings_changed();
            }
            ui.step(0.04);
            let new_run = lock(&shared).seg.run;
            lock(&shared).seg.run = run;
            worker.tick(&shared);
            assert_eq!(lock(&shared).seg.phase, SegPhase::Recording, "reading={reading}, clear={clear}");
            assert_eq!(event("acq_seg_start="), 2);
            assert!(lock(&shared).seg.times.is_empty());
            // Pair publication may reach the worker before the new UI run number.
            lock(&shared).seg.run = new_run;
            worker.tick(&shared);
            assert_eq!(event("acq_seg_start="), 3, "both the old-number arm and its replacement are expected");
        }
    }
}
