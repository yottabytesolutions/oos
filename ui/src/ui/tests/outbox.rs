// SPDX-License-Identifier: GPL-2.0-or-later

use super::super::*;

mod single_reconfigure;

#[test]
fn single_done_stops_in_next_change_set() {
    let mut ui = Ui::for_tests(Source::Demo { t: 0.0 });
    ui.key(key::SINGLE);
    ui.step(0.04);
    assert_eq!(ui.records, 1);
    assert_eq!(ui.m.run, RunState::Run, "completion must leave accepted settings intact");
    let record = ui.last[0].clone().unwrap();
    let id = change::acquisition::identity(&ui);
    ui.step(0.04);
    assert_eq!(ui.m.run, RunState::Stop);
    assert_eq!(ui.records, 1, "pending STOP cannot consume another SINGLE record");
    assert_eq!(ui.last[0].as_ref().unwrap().dt, record.dt);
    assert_eq!(change::acquisition::identity(&ui), id, "STOP alone has no identity boundary");
    assert!(!ui.was_running);
    assert!(ui.stop_final.is_none());
    assert_eq!(change::counters(&ui).4, 0);
}

#[test]
fn segments_done_stops_in_next_change_set() {
    let mut ui = Ui::for_tests(Source::Demo { t: 0.0 });
    ui.m.segments = 2;
    change::reset(&mut ui);
    ui.step(0.04);
    assert_eq!((ui.seg_n, ui.records), (2, 2));
    assert_eq!(ui.m.run, RunState::Run);
    ui.step(0.04);
    assert_eq!(ui.m.run, RunState::Stop);
    assert_eq!((ui.seg_n, ui.records), (2, 2));
    assert_eq!(ui.history.len(), 2);
    assert!(ui.stop_final.is_none());
}

#[test]
fn software_mask_stop_waits_for_next_change_point() {
    let mut ui = Ui::for_tests(Source::Demo { t: 0.0 });
    ui.step(0.04);
    ui.create_mask();
    ui.m.mask_stop = true;
    change::reset(&mut ui);
    let mut failed = ui.last[0].clone().unwrap();
    failed.ch[0].fill(1000.0);
    ui.mask_test(&failed);
    assert!(ui.mask_fail > 0);
    assert_eq!(ui.m.run, RunState::Run);
    let records = ui.records;
    ui.step(0.04);
    assert_eq!(ui.m.run, RunState::Stop);
    assert_eq!(ui.records, records);
    assert!(ui.stop_final.is_none());
}

#[test]
fn internal_writes_apply_before_external_inputs() {
    let mut ui = Ui::for_tests(Source::Demo { t: 0.0 });
    ui.key(key::SINGLE);
    ui.step(0.04);
    assert_eq!(ui.m.run, RunState::Run);
    crate::cmd::run(&mut ui, "key runstop");
    assert_eq!(ui.m.run, RunState::Run, "external toggle must see the queued STOP first");
    assert_eq!(ui.m.trig_mode, TrigMode::Auto);
    let records = ui.records;
    ui.step(0.04);
    assert!(ui.records > records, "external RUN wins over internal completion");
    assert!(ui.host.runtime().change.last.as_ref().unwrap().classes().contains(oos_core::change::Classes::HW_AVERAGE));
    assert_eq!(change::counters(&ui).4, 0);
}

fn hardware() -> (Ui, crate::board::HwShared) {
    hardware_with_host(host::Host::fixture())
}

fn hardware_with_host(host: host::Host) -> (Ui, crate::board::HwShared) {
    let shared = crate::board::HwShared::default();
    let mut ui = Ui::new(Source::Hw { shared: shared.clone(), seen: 0 }, host);
    ui.m.zoom = true;
    ui.m.run = RunState::Stop;
    ui.step(0.04);
    change::reset(&mut ui);
    (ui, shared)
}

fn frame(ui: &Ui, seq: u64, partial: bool) -> crate::board::Frame {
    let id = change::acquisition::identity(ui);
    crate::board::Frame {
        seq,
        cfg: id.gen(),
        stamp: id.epoch(),
        dt: 1e-6,
        pre: 32,
        partial,
        ch: std::array::from_fn(|c| if c == 0 { vec![1.0; 64] } else { Vec::new() }),
        ..Default::default()
    }
}

#[test]
fn partial_single_completes_once_and_leaves_later_records_queued() {
    for separate_steps in [false, true] {
        // The live 15 ms record budget can yield after the partial frame. Exercise both
        // delivery boundaries explicitly, without making them depend on host speed.
        let (mut ui, shared) = hardware_with_host(host::Host::headless());
        ui.key(key::SINGLE);
        ui.step(0.04);
        oos_hal::sync::lock(&shared).queue.push_back(frame(&ui, 1, true).into());
        if separate_steps {
            ui.step(0.04);
            assert_eq!((ui.records, ui.m.run), (1, RunState::Run));
            assert!(ui.last[0].as_ref().unwrap().partial);
            assert!(!change::proposals::stopping(&ui), "partial SINGLE must remain armed");
            ui.step(0.04);
            assert_eq!((ui.records, ui.m.run), (1, RunState::Run), "an empty step cannot complete SINGLE");
        }
        for seq in [2, 3] {
            oos_hal::sync::lock(&shared).queue.push_back(frame(&ui, seq, false).into());
        }
        ui.step(0.04);
        assert_eq!((ui.records, ui.m.run), (2, RunState::Run));
        assert!(change::proposals::stopping(&ui));
        assert_eq!(ui.last[0].as_ref().unwrap().raw.as_ref().unwrap().frame.seq, 2);
        assert_eq!(oos_hal::sync::lock(&shared).queue.front().unwrap().seq, 3);
        let map = ui.ph.maps().clone();
        ui.step(0.04);
        assert_eq!((ui.records, ui.m.run), (2, RunState::Stop));
        assert!(oos_hal::sync::lock(&shared).hold);
        assert!(ui.stop_final.is_none());
        assert!(map.iter().flatten().any(|&v| v != 0));
        assert!(ui.ph.maps().iter().flatten().any(|&v| v != 0), "STOP retains the trace");
        ui.step(0.04);
        assert_eq!((ui.records, ui.m.run), (2, RunState::Stop), "completion must happen once");
        assert!(!change::proposals::stopping(&ui));
        let g = oos_hal::sync::lock(&shared);
        assert_eq!(g.queue.len(), 1);
        assert_eq!(g.queue.front().unwrap().seq, 3);
    }
}

#[test]
fn single_and_run_restart_average_once_per_accepted_boundary() {
    let (mut ui, shared) = hardware();
    for _ in 0..2 {
        let before = ui.m.hw_avg_clear;
        ui.key(key::SINGLE);
        ui.step(0.04);
        assert_eq!(ui.m.hw_avg_clear, before + 1, "SINGLE plus RUN shares one restart");
        assert_eq!(oos_hal::sync::lock(&shared).avg_clear, before + 1);
        ui.step(0.04);
        assert_eq!(ui.m.hw_avg_clear, before + 1, "steady steps do not restart");
    }
    ui.key(key::RUNSTOP);
    ui.step(0.04);
    let before = ui.m.hw_avg_clear;
    ui.key(key::RUNSTOP);
    ui.step(0.04);
    assert_eq!(ui.m.hw_avg_clear, before + 1);
}

#[test]
fn hardware_mask_stop_is_a_deduplicated_internal_command() {
    let (mut ui, shared) = hardware();
    ui.m.run = RunState::Run;
    ui.m.mask_stop = true;
    change::reset(&mut ui);
    ui.step(0.04);
    oos_hal::sync::lock(&shared).mask =
        Some(crate::board::MaskRead { ch: ui.m.mask_src, fails: 1, frames: 1, n: 1, ..Default::default() });
    ui.mask_hw_poll();
    ui.mask_hw_poll();
    assert_eq!(ui.m.run, RunState::Run);
    ui.step(0.04);
    assert_eq!(ui.m.run, RunState::Stop);
    assert!(oos_hal::sync::lock(&shared).hold || ui.last[0].is_none());
    ui.m.run = RunState::Run;
    ui.step(0.04);
    assert_eq!(ui.m.run, RunState::Run, "the same mask reading cannot queue another STOP");
}

#[test]
fn terminal_settings_without_a_stage_token_wait_for_intake() {
    let (mut ui, _) = hardware();
    ui.m.zoom = false;
    ui.step(0.04);
    change::reset(&mut ui);
    let scale = ui.m.tscale;
    change::begin_step(&mut ui, 0.0).unwrap();
    change::proposals::defer(&mut ui, |ui| {
        ui.m.tscale += 1;
        ui.m.trig_level = 1.25;
        crate::ui::change::edited(ui);
    });
    assert_eq!(ui.m.tscale, scale);
    assert_ne!(ui.m.trig_level, 1.25);
    change::finish_step(&mut ui);
    crate::cmd::run(&mut ui, "knob hscale -1");
    assert_eq!(ui.m.tscale, scale + 2, "external relative edits follow the terminal command");
    assert_eq!(ui.m.trig_level, 1.25);
    ui.step(0.04);
    assert_eq!(ui.m.tscale, scale + 2);
    assert_eq!(change::counters(&ui).4, 0);
}

#[test]
fn rejected_internal_payload_keeps_pending_stop_and_the_step_loop() {
    let (mut ui, _) = hardware();
    ui.key(key::SINGLE);
    ui.step(0.04);
    change::proposals::stop(&mut ui);
    let before = ui.m.bode_addr.clone();
    change::proposals::defer(&mut ui, |ui| ui.m.bode_addr = "x".repeat(70 * 1024));
    assert_eq!(ui.m.bode_addr, before, "rejected commands cannot leak into live settings");
    assert!(change::proposals::stopping(&ui));
    let now = ui.now;
    ui.step(0.04);
    assert_eq!(ui.m.run, RunState::Stop, "quarantining another proposal cannot erase completion");
    assert!(ui.now > now);
    assert!(change::text(&ui).contains("PayloadTooLarge="));
    ui.step(0.04);
    assert!(ui.now > now + 0.04);
}

#[test]
fn normal_sweep_with_run_retains_acquisition_and_stats() {
    let (mut ui, _) = hardware();
    ui.m.trig_mode = TrigMode::Auto;
    ui.m.run = RunState::Run;
    ui.step(0.04);
    let mut remote = crate::scpi::Scpi::open(0u16, false);
    remote.exec_line(&mut ui, &Canvas::new(1, 1), ":STOP;:TRIG:SWE NORM;:RUN");
    // the end of the SCPI message is its change point
    let classes = ui.host.runtime().change.last.as_ref().unwrap().classes();
    ui.step(0.04);
    use oos_core::change::Classes as C;
    assert_eq!(ui.m.trig_mode, TrigMode::Normal);
    assert!(classes.contains(C::ACQ | C::STATS | C::HW_AVERAGE));
}

#[test]
fn rejected_step_observes_hardware_segment_completion() {
    let (mut ui, shared) = hardware();
    ui.m.zoom = false;
    ui.m.tscale = tscales().iter().position(|&v| v == 1e-7).unwrap();
    (ui.m.run, ui.m.segments, ui.m.seg_hw) = (RunState::Run, 2, true);
    ui.step(0.04);
    {
        let mut g = oos_hal::sync::lock(&shared);
        (g.seg.done_run, g.seg.phase, g.seg.recorded, g.seg.read) = (ui.seg_run, crate::board::SegPhase::Done, 2, 2);
        g.seg.times = vec![Some(0.0), Some(1e-3)];
    }
    change::duplicate_requests(&mut ui);
    ui.step(0.04);
    assert!(change::text(&ui).contains("DuplicateToken=1"));
    assert_eq!(ui.seg_times, vec![Some(0.0), Some(1e-3)]);
    assert!(change::proposals::stopping(&ui));
    ui.step(0.04);
    assert_eq!(ui.m.run, RunState::Stop);
}

#[test]
fn two_runstop_presses_in_one_frame_restart_the_boundary() {
    let (mut ui, shared) = hardware();
    ui.key(key::RUNSTOP);
    ui.step(0.04);
    let before = (change::acquisition::identity(&ui).epoch(), ui.m.hw_avg_clear);
    ui.key(key::RUNSTOP);
    ui.key(key::RUNSTOP);
    ui.step(0.04);
    assert_eq!(ui.m.run, RunState::Run);
    assert_eq!(change::acquisition::identity(&ui).epoch(), before.0 + 1);
    assert_eq!(oos_hal::sync::lock(&shared).avg_clear, before.1 + 1);
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "internal writer must enter with accepted settings")]
fn deferred_writer_rejects_settings_drift_on_entry() {
    let (mut ui, _) = hardware();
    ui.step(0.04);
    ui.m.trig_level += 1.0;
    change::proposals::defer(&mut ui, |_| {});
}

#[test]
fn idle_writers_leave_unaccepted_edits_at_intake() {
    let (mut ui, _) = hardware();
    ui.step(0.04);
    ui.m.trig_level = 1.25;
    change::proposals::autoset(&mut ui);
    change::proposals::bode(&mut ui);
    ui.step(0.04);
    assert_eq!(ui.m.trig_level, 1.25);
}

#[test]
fn completed_free_bode_restores_settings_on_the_next_intake() {
    let mut ui = Ui::for_tests(Source::Demo { t: 0.0 });
    ui.m.bode_gen = super::super::bode::FREE;
    ui.m.bode_sw_time = 0.05;
    let initial = ui.m.clone();
    ui.activate(Item::BodeRun);
    assert!(ui.bode.running);
    ui.step(0.001);
    assert!(ui.bode.running);
    let active = ui.m.clone();
    assert_ne!(active.tscale, initial.tscale);
    change::proposals::defer(&mut ui, |ui| ui.bode_finish("Test sweep complete"));
    assert!(!ui.bode.running);
    assert_eq!((ui.m.tscale, ui.m.trig_mode), (active.tscale, active.trig_mode), "restoration is deferred");
    ui.step(0.001);
    assert_eq!((ui.m.tscale, ui.m.trig_mode, ui.m.auto_roll), (initial.tscale, initial.trig_mode, initial.auto_roll));
    ui.step(0.001);
    assert_eq!((ui.m.tscale, ui.m.trig_mode), (initial.tscale, initial.trig_mode));
}

#[test]
fn one_scpi_message_is_one_accepted_change_with_the_union_of_classes() {
    use oos_core::change::Classes as C;
    let accepted = |messages: &[&str]| {
        let (mut ui, _) = hardware();
        ui.m.trig_mode = TrigMode::Auto;
        ui.m.run = RunState::Run;
        ui.step(0.04);
        let mut remote = crate::scpi::Scpi::open(0u16, false);
        let mut union = C::NONE;
        for message in messages {
            remote.exec_line(&mut ui, &Canvas::new(1, 1), message);
            union |= ui.host.runtime().change.last.as_ref().unwrap().classes();
        }
        (ui.host.runtime().change.last.as_ref().unwrap().classes(), union)
    };
    let (one, _) = accepted(&[":STOP;:TRIG:SWE NORM;:RUN"]);
    assert_eq!(one, C::ACQ | C::STATS | C::HW_AVERAGE);
    let (last, union) = accepted(&[":STOP", ":TRIG:SWE NORM", ":RUN"]);
    assert_eq!(union, one, "three messages accept the same classes as three changes");
    assert_ne!(last, one);
}
