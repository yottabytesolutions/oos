// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::board::scripted::{BatchKind, SingleWorker};
use oos_hal::sync::lock;

fn setup(kind: BatchKind) -> (Ui, crate::board::HwShared, SingleWorker) {
    let shared = crate::board::HwShared::default();
    let mut ui = Ui::for_tests(Source::Hw { shared: shared.clone(), seen: 0 });
    ui.m.run = RunState::Stop;
    ui.m.zoom = false;
    ui.m.tscale = tscales()
        .iter()
        .position(|&v| v == if matches!(kind, BatchKind::Roll | BatchKind::Scan) { 0.2 } else { 1e-6 })
        .unwrap();
    ui.m.auto_roll = kind == BatchKind::Roll;
    if kind == BatchKind::DeepPairs {
        ui.m.mdepth = 5;
    }
    ui.relayout();
    change::reset(&mut ui);
    ui.step(0.04);
    ui.key(key::SINGLE);
    ui.step(0.04);
    (ui, shared, SingleWorker::new(kind))
}

fn event(name: &str) -> u64 {
    crate::drawpipe::counters()
        .acquisition
        .text()
        .split_whitespace()
        .find_map(|s| s.strip_prefix(name))
        .unwrap()
        .parse()
        .unwrap()
}

fn finish(ui: &mut Ui, shared: &crate::board::HwShared, worker: &mut SingleWorker, frame: crate::board::Frame) {
    let pair = (frame.cfg, frame.stamp);
    let expected = ui.records + 1;
    SingleWorker::deliver(shared, frame);
    ui.step(0.04);
    assert_eq!(ui.records, expected);
    assert!(change::proposals::stopping(ui));
    assert_eq!(ui.m.run, RunState::Run, "completion still goes through the outbox");
    let record = &ui.last[0].as_ref().unwrap().raw.as_ref().unwrap().frame;
    assert_eq!((record.cfg, record.stamp), pair);
    if lock(shared).slow == crate::board::Slow::Off {
        assert!(worker.read(shared, &ui.m, ui.switches()).is_none(), "no extra arm before STOP intake");
    }
    let arms = event("acq_single_arm=");
    ui.step(0.04);
    assert_eq!(ui.m.run, RunState::Stop);
    for _ in 0..20 {
        assert!(worker.read(shared, &ui.m, ui.switches()).is_none());
        ui.step(0.04);
    }
    assert_eq!((ui.records, event("acq_single_arm=")), (expected, arms));
}

#[test]
fn incomplete_single_reconfigures_before_late_old_pair_record() {
    for kind in [BatchKind::Raw, BatchKind::Roll, BatchKind::Scan, BatchKind::DeepPairs] {
        for late in [false, true] {
            let _scope = crate::drawpipe::test_scope::Scope::new();
            let (mut ui, shared, mut worker) = setup(kind);
            let old = worker.read(&shared, &ui.m, ui.switches()).unwrap();
            assert_eq!(event("acq_single_arm="), u64::from(matches!(kind, BatchKind::Raw | BatchKind::DeepPairs)));
            if !late {
                SingleWorker::deliver(&shared, old.clone());
            }
            ui.m.tscale += 1;
            ui.settings_changed();
            ui.step(0.03);
            let id = change::acquisition::identity(&ui);
            assert_ne!(id.gen(), old.cfg);
            assert_eq!(id.epoch(), old.stamp, "generation-only reconfigure");
            if late {
                SingleWorker::deliver(&shared, old);
            }
            let losses = &crate::drawpipe::counters().losses;
            use std::sync::atomic::Ordering::Relaxed;
            assert_eq!(losses.ui_publish.load(Relaxed), u64::from(!late));
            assert_eq!(losses.worker_push.load(Relaxed), u64::from(late));
            assert_eq!(ui.records, 0);
            let next = worker.read(&shared, &ui.m, ui.switches()).expect("obsolete batch cannot finish SINGLE");
            assert_eq!((next.cfg, next.stamp), (id.gen(), id.epoch()));
            assert_eq!(event("acq_single_arm="), 2 * u64::from(matches!(kind, BatchKind::Raw | BatchKind::DeepPairs)));
            assert_eq!(event("acq_configure="), 2);
            finish(&mut ui, &shared, &mut worker, next);
        }
    }
}

#[test]
fn completed_single_does_not_rearm_after_reconfigure() {
    for kind in [BatchKind::Raw, BatchKind::Roll, BatchKind::Scan, BatchKind::DeepPairs] {
        let _scope = crate::drawpipe::test_scope::Scope::new();
        let (mut ui, shared, mut worker) = setup(kind);
        let frame = worker.read(&shared, &ui.m, ui.switches()).unwrap();
        SingleWorker::deliver(&shared, frame);
        ui.step(0.04);
        assert!(change::proposals::stopping(&ui));
        let arms = event("acq_single_arm=");
        ui.m.tscale += 1;
        ui.settings_changed();
        ui.step(0.04);
        assert!(lock(&shared).hold, "completion STOP is published before changed settings");
        for _ in 0..20 {
            assert!(lock(&shared).hold, "completed STOP must stay held: {kind:?}");
            assert!(worker.read(&shared, &ui.m, ui.switches()).is_none(), "held {kind:?}");
            ui.step(0.04);
        }
        assert_eq!((ui.records, ui.m.run, event("acq_single_arm=")), (1, RunState::Stop, arms));
    }
}

#[test]
fn incomplete_single_retries_after_epoch_only_clear() {
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let (mut ui, shared, mut worker) = setup(BatchKind::Raw);
    let old = worker.read(&shared, &ui.m, ui.switches()).unwrap();
    SingleWorker::deliver(&shared, old.clone());
    ui.key(key::CLEAR);
    ui.step(0.03);
    let id = change::acquisition::identity(&ui);
    assert_eq!(id.gen(), old.cfg);
    assert_ne!(id.epoch(), old.stamp);
    assert_eq!(ui.records, 0);
    let next = worker.read(&shared, &ui.m, ui.switches()).unwrap();
    assert_eq!(event("acq_single_arm="), 2);
    assert_eq!(event("acq_configure="), 1, "CLEAR does not configure hardware");
    finish(&mut ui, &shared, &mut worker, next);
}

#[test]
fn twenty_single_presses_replace_obsolete_batches_once_each() {
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let (mut ui, shared, mut worker) = setup(BatchKind::Raw);
    let mut remote = crate::scpi::Scpi::open(0u16, false);
    let canvas = Canvas::new(1, 1);
    for press in 0..20 {
        if press > 0 {
            remote.exec_line(&mut ui, &canvas, ":SINGle");
            ui.step(0.04);
        }
        let old = worker.read(&shared, &ui.m, ui.switches()).unwrap();
        SingleWorker::deliver(&shared, old);
        remote.exec_line(
            &mut ui,
            &canvas,
            if press % 2 == 0 { ":TIMebase:SCALe 0.000002" } else { ":TIMebase:SCALe 0.000001" },
        );
        ui.step(0.03);
        assert_eq!(ui.records, press);
        let next = worker.read(&shared, &ui.m, ui.switches()).unwrap();
        finish(&mut ui, &shared, &mut worker, next);
        assert_eq!(event("acq_single_arm="), 2 * (press + 1));
        assert_eq!(ui.records, press + 1);
    }
}

#[test]
fn completed_single_stays_held_after_clear() {
    for kind in [BatchKind::Raw, BatchKind::Roll, BatchKind::Scan, BatchKind::DeepPairs] {
        let _scope = crate::drawpipe::test_scope::Scope::new();
        let (mut ui, shared, mut worker) = setup(kind);
        let frame = worker.read(&shared, &ui.m, ui.switches()).unwrap();
        SingleWorker::deliver(&shared, frame);
        ui.step(0.04);
        ui.key(key::CLEAR);
        ui.step(0.04);
        assert_eq!(ui.m.run, RunState::Stop);
        assert!(ui.cleared && ui.last[0].is_none());
        for _ in 0..20 {
            assert!(worker.read(&shared, &ui.m, ui.switches()).is_none(), "completed then CLEAR: {kind:?}");
            ui.step(0.04);
        }
        assert_eq!(ui.records, 1);
    }
}

#[test]
fn incomplete_single_survives_a_stream_mode_reconfigure() {
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let (mut ui, shared, mut worker) = setup(BatchKind::Scan);
    let old = worker.read(&shared, &ui.m, ui.switches()).unwrap();
    let want = lock(&shared).want.clone();
    ui.m.auto_roll = true;
    ui.settings_changed();
    ui.step(0.03);
    assert_eq!(lock(&shared).slow, crate::board::Slow::Roll);
    assert_eq!(change::counters(&ui).4, 0, "mode-only reconfiguration is an observed acquisition boundary");
    assert_eq!(lock(&shared).want, want, "mode alone changes the hardware configuration");
    SingleWorker::deliver(&shared, old);
    let next = worker.read(&shared, &ui.m, ui.switches()).unwrap();
    assert_eq!((event("acq_configure="), event("acq_roll_start="), event("acq_single_arm=")), (2, 2, 0));
    finish(&mut ui, &shared, &mut worker, next);
}

#[test]
fn incomplete_single_retries_a_gen_boundary_without_hardware_reconfigure() {
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let (mut ui, shared, mut worker) = setup(BatchKind::Raw);
    let old = worker.read(&shared, &ui.m, ui.switches()).unwrap();
    let want = lock(&shared).want.clone();
    SingleWorker::deliver(&shared, old.clone());
    ui.m.mask_src = 1;
    ui.settings_changed();
    ui.step(0.03);
    let id = change::acquisition::identity(&ui);
    assert_ne!(id.gen(), old.cfg);
    assert_eq!(id.epoch(), old.stamp);
    assert_eq!(lock(&shared).want, want);
    let next = worker.read(&shared, &ui.m, ui.switches()).unwrap();
    assert_eq!((event("acq_configure="), event("acq_single_arm=")), (1, 2));
    finish(&mut ui, &shared, &mut worker, next);
}

#[test]
fn scan_run_overrides_completion_without_an_acquisition_disagreement() {
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let (mut ui, shared, mut worker) = setup(BatchKind::Scan);
    let frame = worker.read(&shared, &ui.m, ui.switches()).unwrap();
    SingleWorker::deliver(&shared, frame);
    ui.step(0.04);
    assert!(change::proposals::stopping(&ui));
    let id = change::acquisition::identity(&ui);
    crate::cmd::run(&mut ui, "key runstop");
    ui.step(0.04);
    assert_eq!((ui.m.run, ui.m.trig_mode), (RunState::Run, TrigMode::Auto));
    assert_eq!(change::acquisition::identity(&ui).gen(), id.gen(), "RUN's sweep companion is not an acquisition edit");
    assert_eq!(change::counters(&ui).4, 0);
    assert!(worker.read(&shared, &ui.m, ui.switches()).is_some(), "an explicit RUN releases completion hold");
}

mod segmented;

#[test]
fn stop_after_clear_in_run_before_any_record_holds_the_worker() {
    for kind in [BatchKind::Raw, BatchKind::Roll, BatchKind::Scan, BatchKind::DeepPairs] {
        let _scope = crate::drawpipe::test_scope::Scope::new();
        let (mut ui, shared, mut worker) = setup(kind);
        let mut remote = crate::scpi::Scpi::open(0u16, false);
        remote.exec_line(&mut ui, &Canvas::new(1, 1), ":RUN;:CLEar");
        ui.step(0.04);
        assert_eq!((ui.m.run, ui.records), (RunState::Run, 0));
        remote.exec_line(&mut ui, &Canvas::new(1, 1), ":STOP");
        ui.step(0.04);
        assert!(lock(&shared).hold && ui.cleared && ui.last[0].is_none());
        for _ in 0..20 {
            assert!(worker.read(&shared, &ui.m, ui.switches()).is_none());
            ui.step(0.04);
        }
        assert_eq!((ui.records, event("acq_single_arm="), event("acq_roll_start=")), (0, 0, 0));
    }
}

#[test]
fn scan_run_stops_its_stream_when_stop_is_accepted() {
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let (mut ui, shared, mut worker) = setup(BatchKind::Scan);
    let mut remote = crate::scpi::Scpi::open(0u16, false);
    remote.exec_line(&mut ui, &Canvas::new(1, 1), ":RUN");
    ui.step(0.04);
    let frame = worker.read(&shared, &ui.m, ui.switches()).unwrap();
    SingleWorker::deliver(&shared, frame);
    ui.step(0.04);
    assert_eq!((ui.m.run, ui.records), (RunState::Run, 1));
    let starts = event("acq_roll_start=");
    remote.exec_line(&mut ui, &Canvas::new(1, 1), ":STOP");
    ui.step(0.04);
    for _ in 0..20 {
        assert!(worker.read(&shared, &ui.m, ui.switches()).is_none());
        ui.step(0.04);
    }
    assert_eq!((ui.m.run, ui.records, event("acq_roll_start=")), (RunState::Stop, 1, starts));
    assert_eq!(event("acq_roll_stop="), 1);
}
