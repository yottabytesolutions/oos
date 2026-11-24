// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use oos_core::change::Classes;
use oos_hal::sync::lock;

fn candidate(model: &Model, sw: &crate::sysconf::Switches) -> crate::board::Want {
    let mut model = model.clone();
    model.trig_type = TrigType::Edge;
    model.trig_mode = TrigMode::Auto;
    model.trig_src = model.dec_src;
    model.trig_ref = None;
    model.trig_level = model.dec_thr;
    model.slope = Slope::Rise;
    board().want(&model, sw)
}

#[test]
fn mixed_bus_publication_is_ready_before_record_intake() {
    let (mut ui, shared) = fixture(10_000);
    let before = change::acquisition::identity(&ui);
    let id = change::acquisition::script_prepare(&mut ui);
    let g = lock(&shared);
    assert_eq!(g.want, Some(candidate(&ui.m, ui.switches())));
    assert_eq!(g.acquisition.identity(), id);
    assert_eq!(id.gen(), before.gen() + 1);
    assert_eq!(ui.m.trig_type, TrigType::Bus);
    assert_eq!(ui.m.trig_mode, TrigMode::Normal);
    assert_eq!(g.slow, crate::board::Slow::Off);
}

#[test]
fn mixed_bus_intake_cannot_publish_without_generation_or_class() {
    let (mut ui, shared) = fixture(10_000);
    ui.step(0.001);
    let before = change::acquisition::identity(&ui);
    let want = lock(&shared).want.clone();
    ui.m.dec_thr = 1.2;
    assert!(ui.acquire_mixed_bus_record(0.001, &mut 0).is_none());
    assert_eq!(lock(&shared).want, want, "record intake bypassed classification and generation");
    assert_eq!(change::acquisition::identity(&ui), before);
    ui.step(0.001);
    let changes = ui.host.runtime().change.last.as_ref().unwrap();
    assert_eq!(changes.identity().gen(), before.gen() + 1);
    assert!(changes.classes().contains(Classes::ACQ | Classes::STATS));
    assert_eq!(lock(&shared).want, Some(candidate(&ui.m, ui.switches())));
    let id = changes.identity();
    ui.step(0.001);
    assert_eq!(change::acquisition::identity(&ui), id, "unchanged candidate policy must be idempotent");
}

#[test]
fn mixed_bus_intake_rejects_old_generation_at_current_epoch() {
    let (mut ui, shared) = fixture(10_000);
    let id = change::acquisition::script_prepare(&mut ui);
    let mut old = (*frame(10_000, 0xa5, 2, id)).clone();
    old.cfg = id.gen() - 1;
    lock(&shared).queue.push_back(old.into());
    let mut filtered = 0;
    assert!(
        ui.acquire_mixed_bus_record(0.001, &mut filtered).is_none(),
        "old generation reached the mixed-source decoder"
    );
    lock(&shared).queue.push_back((*frame(10_000, 0xa5, 3, id)).clone().into());
    let record = ui.acquire_mixed_bus_record(0.001, &mut filtered).unwrap();
    assert_eq!(filtered, 1);
    let raw = &record.raw.as_ref().unwrap().frame;
    assert_eq!((raw.seq, raw.cfg, raw.stamp), (3, id.gen(), id.epoch()));
    assert_eq!((raw.dt, raw.pre), (1e-9, 5000), "preserve hardware/reference time alignment");
}

#[test]
fn mixed_bus_policy_changes_are_acquisition_and_statistics_changes() {
    let model = Model::default();
    let normal = oos_core::change::UiAcq::default();
    let mixed = normal.with_mixed_bus(true);
    for (before, after) in [(normal, mixed), (mixed, normal)] {
        assert_eq!(oos_core::change::classify(&model, &model, before, after), Classes::ACQ | Classes::STATS);
    }
    assert_eq!(oos_core::change::classify(&model, &model, mixed, mixed), Classes::NONE);
}

#[test]
fn mixed_bus_single_and_stop_use_accepted_transition_effects() {
    let (mut ui, shared) = fixture(10_000);
    ui.step(0.001);
    let before = change::acquisition::identity(&ui);
    let avg = lock(&shared).avg_clear;
    assert!(ui.mixed_bus_command(2));
    ui.step(0.001);
    let armed = change::acquisition::identity(&ui);
    assert_eq!(armed.gen(), before.gen(), "SINGLE is an epoch request, not a candidate reconfigure");
    assert_eq!(armed.epoch(), before.epoch() + 1);
    assert_eq!(lock(&shared).avg_clear, avg + 1);
    assert!(!lock(&shared).hold);
    assert_eq!(lock(&shared).want, Some(candidate(&ui.m, ui.switches())));
    assert!(ui.mixed_bus_command(0));
    ui.step(0.001);
    assert!(lock(&shared).hold, "STOP hold must precede publication");
    assert_eq!(lock(&shared).acquisition.identity(), change::acquisition::identity(&ui));
    let stopped = change::acquisition::identity(&ui);
    assert!(ui.mixed_bus_command(1));
    ui.step(0.001);
    assert!(!lock(&shared).hold);
    assert_eq!(change::acquisition::identity(&ui).epoch(), stopped.epoch() + 1);
}

#[test]
fn mixed_bus_result_keeps_live_reference_alignment_and_deferred_single_stop() {
    let (mut ui, shared) = fixture(10_000);
    assert!(ui.mixed_bus_command(2));
    ui.step(0.001);
    let before = ui.records;
    push(&mut ui, &shared, 10_000, 0x5a, 2);
    ui.step(0.001);
    until(&mut ui, |ui| !ui.bus_candidate_waiting());
    assert_eq!(ui.records, before, "an unmatched Edge/Auto candidate must not complete Bus SINGLE");
    push(&mut ui, &shared, 10_000, 0xa5, 3);
    let id = change::acquisition::identity(&ui);
    ui.step(0.001);
    until(&mut ui, |ui| ui.records == before + 1);
    let record = ui.last[0].as_ref().unwrap();
    assert!(record.triggered);
    let raw = record.raw.as_ref().unwrap();
    assert_eq!((raw.frame.seq, raw.frame.cfg, raw.frame.stamp), (3, id.gen(), id.epoch()));
    assert!((raw.t0 - raw.i0 as f64 * raw.frame.dt - ui.refs[0].as_ref().unwrap().t0).abs() < 1e-15);
    assert!(ui.bus_candidate.last_hit.is_some());
    assert_eq!(ui.m.run, RunState::Run, "completion STOP waits in the outbox until next intake");
    ui.step(0.001);
    assert_eq!(ui.m.run, RunState::Stop);
    assert!(lock(&shared).hold);
    assert_eq!(ui.records, before + 1);
}
