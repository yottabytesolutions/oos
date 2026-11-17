// SPDX-License-Identifier: GPL-2.0-or-later

use super::super::*;
use oos_core::change::Classes as C;

// These views calculate from the current record and mapping without a temporal buffer.
const XY_RESETS_ON: C = C::NONE;
const CURSOR_RESETS_ON: C = C::NONE;
const MATH_RESETS_ON: C = C::NONE;

fn base(run: RunState) -> Ui {
    let mut ui = Ui::new(Source::Hw { shared: Default::default(), seen: 0 }, host::Host::headless());
    ui.m.run = run;
    ui.step(0.04);
    change::reset(&mut ui);
    ui
}

#[test]
fn effective_statistics_change_resets_without_a_mark_in_run_and_stop() {
    for run in [RunState::Run, RunState::Stop] {
        let mut ui = base(run);
        ui.mstat = vec![MStat { n: 7, ..Default::default() }];
        ui.meas_new = true;
        ui.m.ch[0].offset += 0.1;
        change::begin_step(&mut ui, 0.04).unwrap();
        assert!(ui.mstat.is_empty());
        assert!(!ui.meas_new, "an old notification cannot refill reset statistics");
        ui.mstat = vec![MStat { n: 9, ..Default::default() }];
        change::begin_step(&mut ui, 0.04).unwrap();
        assert_eq!(ui.mstat[0].n, 9, "one acting reset per accepted diff");
    }
}

#[test]
fn unrelated_and_equal_input_preserves_statistics() {
    let mut ui = base(RunState::Stop);
    for delta in [0, 1] {
        ui.mstat = vec![MStat { n: 7, ..Default::default() }];
        ui.adjust(Item::GridBright, delta);
        change::begin_step(&mut ui, 0.04).unwrap();
        assert_eq!(ui.mstat.first().map(|s| s.n), Some(7));
    }
}

#[test]
fn hardware_average_restarts_once_for_class_union() {
    let mut ui = base(RunState::Run);
    let clear = ui.m.hw_avg_clear;
    ui.m.acq = AcqMode::Average;
    ui.m.avg_count += 1;
    change::begin_step(&mut ui, 0.04).unwrap();
    assert_eq!(ui.m.hw_avg_clear, clear + 1);
    change::begin_step(&mut ui, 0.04).unwrap();
    assert_eq!(ui.m.hw_avg_clear, clear + 1);
}

#[test]
fn history_class_clears_stores_even_when_disabling_segments() {
    let mut ui = base(RunState::Stop);
    ui.m.segments = 10;
    change::begin_step(&mut ui, 0.04).unwrap();
    ui.history.push_back(Ui::blank_record());
    ui.hist_pos = Some(0);
    ui.seg_times = vec![Some(1.0)];
    ui.seg_pos = Some(0);
    ui.seg_n = 3;
    ui.m.segments = 0;
    change::begin_step(&mut ui, 0.04).unwrap();
    assert!(ui.history.is_empty() && ui.hist_pos.is_none());
    assert!(ui.seg_times.is_empty() && ui.seg_pos.is_none());
    assert_eq!(ui.seg_n, 0);
}

#[test]
fn mask_and_zone_classes_reset_once_without_input_side_effects() {
    let mut ui = base(RunState::Stop);
    (ui.mask_pass, ui.mask_fail, ui.zone_drops) = (11, 12, 13);
    let (mask, zone) = (ui.mask_hw_clear, ui.zone_hw_clear);
    ui.m.mask_x += 1.0;
    ui.m.zones[0].on = true;
    change::begin_step(&mut ui, 0.04).unwrap();
    assert_eq!((ui.mask_pass, ui.mask_fail, ui.zone_drops), (0, 0, 0));
    assert_eq!((ui.mask_hw_clear, ui.zone_hw_clear), (mask + 1, zone + 1));
    change::begin_step(&mut ui, 0.04).unwrap();
    assert_eq!((ui.mask_hw_clear, ui.zone_hw_clear), (mask + 1, zone + 1));
}

#[test]
fn hidden_analyzer_state_resets_on_accepted_view_class() {
    let mut ui = base(RunState::Stop);
    let mut eye = oos_dsp::eye::Eye::new(4, 4, 2, -1.0, 1.0);
    eye.records = 7;
    ui.azr.eye = Some(eye);
    ui.azr.spec_avg = Some(Default::default());
    ui.m.eye_rate *= 2.0;
    if ui.m.eye_rate == 0.0 {
        ui.m.eye_rate = 1e6;
    }
    change::begin_step(&mut ui, 0.04).unwrap();
    assert!(ui.host.runtime().change.last.as_ref().unwrap().classes().contains(C::VIEW));
    assert!(ui.azr.eye.is_none());
    assert!(ui.azr.spec_avg.is_none());
}

#[test]
fn declared_reset_classes_match_every_view() {
    for c in C::ALL.into_iter().chain([C::NONE]) {
        let mut ui = base(RunState::Stop);
        let mut trend = panes::Trend::default();
        trend.push(0.0, 42.0);
        ui.trend.push(trend);
        let mut eye = oos_dsp::eye::Eye::new(4, 4, 2, -1.0, 1.0);
        eye.records = 7;
        ui.azr.eye = Some(eye);
        ui.azr.spec_avg = Some(Default::default());
        let mut wf = oos_dsp::spectrum::Waterfall::new(4);
        wf.rows.push_back(vec![1.0]);
        ui.azr.wf = Some(wf);
        ui.azr.power = vec!["held".into()];
        let record = Ui::blank_record();
        ui.last[0] = Some(record.clone());
        ui.m.cursor = CursorMode::Time;
        let cursors = (ui.m.cur_a, ui.m.cur_b, ui.m.cur_c, ui.m.cur_d);
        ui.mstat = vec![MStat { n: 7, ..Default::default() }];
        ui.history.push_back(record);
        (ui.mask_pass, ui.mask_fail, ui.zone_drops) = (1, 2, 3);
        let average = ui.m.hw_avg_clear;
        change::consumers::apply_classes(&mut ui, c);
        let trend_reset = c == C::ACQ || c == C::STATS;
        let eye_reset = c == C::MAPPING || c == C::VIEW;
        let analysis_reset = c == C::MATH || c == C::VIEW;
        assert_eq!(panes::TREND_RESETS_ON, C::ACQ | C::STATS);
        assert_eq!(eyepage::EYE_RESETS_ON, C::MAPPING | C::VIEW);
        assert_eq!(analysis::RESETS_ON, C::MATH | C::VIEW);
        assert_eq!((XY_RESETS_ON, CURSOR_RESETS_ON, MATH_RESETS_ON), (C::NONE, C::NONE, C::NONE));
        assert_eq!(eyepage::automatic::RESETS_ON, C::NONE);
        assert_eq!(eyepage::REFERENCE_RESETS_ON, C::VIEW);
        assert_eq!(ui.trend.is_empty(), trend_reset, "trend {c:?}");
        assert_eq!(ui.azr.eye.is_none(), eye_reset, "eye {c:?}");
        assert_eq!(ui.azr.spec_avg.is_none(), analysis_reset, "spectrum {c:?}");
        assert_eq!(ui.azr.wf.as_ref().unwrap().rows.is_empty(), analysis_reset, "waterfall {c:?}");
        assert_eq!(ui.azr.power.is_empty(), analysis_reset, "power {c:?}");
        assert_eq!(ui.mstat.is_empty(), c == C::STATS, "stats {c:?}");
        assert_eq!(ui.history.is_empty(), c == C::HISTORY, "history {c:?}");
        assert_eq!(ui.mask_pass, if c == C::MASK { 0 } else { 1 });
        assert_eq!(ui.zone_drops, if c == C::ZONE { 0 } else { 3 });
        assert_eq!(ui.m.hw_avg_clear, average + u64::from(c == C::HW_AVERAGE));
        assert_eq!((ui.m.cur_a, ui.m.cur_b, ui.m.cur_c, ui.m.cur_d), cursors);
        assert!(ui.last[0].is_some(), "stateless XY/math/cursors read the retained record");
    }
}

#[test]
fn statistics_reset_request_acts_at_equal_settings_and_only_at_acceptance() {
    let mut ui = base(RunState::Stop);
    for _ in 0..2 {
        ui.mstat = vec![MStat { n: 7, ..Default::default() }];
        ui.stats_reset();
        assert_eq!(ui.mstat[0].n, 7, "input queues the request");
        change::begin_step(&mut ui, 0.04).unwrap();
        assert!(ui.mstat.is_empty());
    }
}

#[test]
fn script_preparation_applies_resets_before_publication_once() {
    let mut ui = base(RunState::Stop);
    let before = ui.m.hw_avg_clear;
    ui.m.acq = AcqMode::Average;
    let id = change::acquisition::script_prepare(&mut ui);
    assert_eq!(ui.m.hw_avg_clear, before + 1);
    let Source::Hw { shared, .. } = &ui.src else { unreachable!() };
    assert_eq!(oos_hal::sync::lock(shared).avg_clear, before + 1);
    ui.step(0.04);
    assert_eq!(change::acquisition::identity(&ui), id);
    assert_eq!(ui.m.hw_avg_clear, before + 1, "step must not consume the prepared union twice");
}

#[test]
fn explicit_view_reset_repeats_at_equal_settings_without_statistics_reset() {
    let mut ui = base(RunState::Stop);
    for _ in 0..2 {
        ui.mstat = vec![MStat { n: 7, ..Default::default() }];
        let mut eye = oos_dsp::eye::Eye::new(4, 4, 2, -1.0, 1.0);
        eye.records = 7;
        ui.azr.eye = Some(eye);
        ui.activate(Item::EyeClear);
        assert!(ui.azr.eye.is_some(), "input queues the reset");
        change::begin_step(&mut ui, 0.04).unwrap();
        assert!(ui.azr.eye.is_none());
        assert_eq!(ui.mstat[0].n, 7);
    }
}

#[test]
fn reference_and_snapshot_eye_declarations_retain_native_geometry() {
    for snapshot in [false, true] {
        for c in C::ALL.into_iter().chain([C::NONE]) {
            let mut ui = base(RunState::Stop);
            ui.m.eye_src = WaveId::Ref(0);
            ui.azr.eye_auto.enabled = snapshot;
            let mut eye = oos_dsp::eye::Eye::new(4, 4, 2, -1.0, 1.0);
            eye.records = 7;
            ui.azr.eye = Some(eye);
            change::consumers::apply_classes(&mut ui, c);
            assert_eq!(ui.azr.eye.is_none(), !snapshot && c == C::VIEW, "snapshot={snapshot}, {c:?}");
            if let Some(eye) = &ui.azr.eye {
                assert_eq!(eye.records, 7, "scope mapping cannot change native record history");
            }
        }
    }
}

#[test]
fn stopped_vertical_and_timebase_edits_keep_current_measurements() {
    let mut ui = Ui::for_tests(Source::Demo { t: 0.0 });
    ui.step(0.04);
    ui.key(key::RUNSTOP);
    ui.step(0.04);
    assert_eq!(ui.m.run, RunState::Stop);
    assert!(ui.stats[0].valid);
    let scale = ui.m.tscale;
    for timebase in [false, true] {
        for _ in 0..40 {
            let stats = ui.stats[0];
            let analysis = ui.ana[0].clone();
            ui.mstat = vec![MStat { n: 7, ..Default::default() }];
            if timebase {
                ui.m.tscale = if ui.m.tscale == scale { scale + 1 } else { scale };
            } else {
                ui.m.ch[0].offset += 0.01;
            }
            change::begin_step(&mut ui, 0.04).unwrap();
            change::finish_step(&mut ui);
            assert!(ui.stats[0].valid, "STOP edits retain current values");
            assert_eq!(ui.stats[0].vavg, stats.vavg);
            assert!(std::sync::Arc::ptr_eq(&ui.ana[0], &analysis));
            assert!(ui.mstat.is_empty(), "only accumulated statistics reset");
            ui.step(0.04);
            assert!(ui.stats[0].valid, "ordinary STOP preview keeps valid values");
            let stats = ui.stats[0];
            ui.m.trig_level = 100.0;
            ui.level_50();
            assert_eq!(ui.m.trig_level, crate::wave::mid_level(&stats));
        }
    }
}

#[test]
fn stats_class_retains_pending_measurements_but_clear_removes_values() {
    let mut ui = base(RunState::Stop);
    let record = Ui::blank_record();
    ui.last[0] = Some(record.clone());
    ui.meas_pending = Some(record);
    ui.meas_want = true;
    ui.stats[0].valid = true;
    let (tx, _jobs) = std::sync::mpsc::channel();
    let (_done, rx) = std::sync::mpsc::channel();
    let (_analysis, analysis_rx) = std::sync::mpsc::channel();
    ui.meas_bg = Some(MeasWorker {
        tx,
        rx: rx.into(),
        analysis_rx: analysis_rx.into(),
        analysis_busy: false,
        busy: true,
        accept: true,
        join: std::thread::spawn(|| {}),
    });
    change::consumers::apply_classes(&mut ui, C::STATS);
    assert!(ui.meas_bg.as_ref().unwrap().accept, "identity gates the in-flight result");
    assert!(ui.meas_bg.as_ref().unwrap().busy);
    assert!(ui.meas_pending.is_some());
    assert!(ui.meas_want);
    assert!(ui.stats[0].valid);
    ui.clear_display();
    change::begin_step(&mut ui, 0.04).unwrap();
    assert!(!ui.stats[0].valid);
    assert!(ui.meas_pending.is_none());
    assert!(!ui.meas_want);
    assert!(!ui.meas_bg.as_ref().unwrap().accept, "CLEAR cancels the in-flight result");
}

#[test]
fn playback_position_keeps_only_the_reference_eye_until_an_ordinary_view_reset() {
    for reference in [false, true] {
        for view_edit in [false, true] {
            let mut ui = base(RunState::Stop);
            ui.m.eye_src = if reference { WaveId::Ref(0) } else { WaveId::Ch(0) };
            change::reset(&mut ui);
            let mut eye = oos_dsp::eye::Eye::new(4, 4, 2, -1.0, 1.0);
            eye.records = 7;
            ui.azr.eye = Some(eye);
            let position = ui.m.delay + 0.0001;
            change::observations::playback_position(&mut ui, position);
            if view_edit {
                ui.m.eye_rate = 1e6;
            }
            change::begin_step(&mut ui, 0.0).unwrap();
            change::finish_step(&mut ui);
            assert!(ui.host.runtime().change.last.as_ref().unwrap().classes().contains(C::VIEW));
            assert_eq!(ui.azr.eye.is_some(), reference && !view_edit);
            if let Some(eye) = &ui.azr.eye {
                assert_eq!(eye.records, 7, "driver navigation retains the accumulated reference Eye");
                ui.activate(Item::EyeClear);
                change::begin_step(&mut ui, 0.0).unwrap();
                assert!(ui.azr.eye.is_none(), "explicit VIEW reset still clears a reference Eye");
            }
        }
    }
}

// Break it catches: statistics that keep the records from before their display went on (the measure page golden
// shows n 24 instead of 21). Turning statistics on counts from that moment, on the panel, SCPI and workspace paths.
type Switch = (&'static str, fn(&mut Ui));

#[test]
fn statistics_count_from_the_moment_their_display_goes_on() {
    let switches: [Switch; 3] = [
        ("panel", |ui| ui.activate(Item::MeasStats)),
        ("scpi", |ui| {
            crate::scpi::Scpi::open(0u16, false).exec_line(ui, &Canvas::new(1, 1), ":MEAS:STAT:DISP ON");
        }),
        ("workspace", |ui| ui.workspace_action(workspace::Action::MeasureFloat)),
    ];
    for (path, switch_on) in switches {
        let mut ui = Ui::for_tests(Source::Demo { t: 0.0 });
        ui.meas_add(Meas::Vpp, WaveId::Ch(0)).expect("a free row");
        for _ in 0..3 {
            ui.step(0.05);
        }
        assert!(!ui.stats_shown());
        assert!(ui.measurement_statistics_snapshot_for_test()[0].0 > 0, "{path}: counting before the switch");
        switch_on(&mut ui);
        assert!(ui.stats_shown(), "{path}");
        for _ in 0..5 {
            ui.step(0.05);
        }
        let count = ui.measurement_statistics_snapshot_for_test()[0].0;
        assert_eq!(count, 5, "{path}: the five records after the switch");
    }
}

// integ-27 port 1 (81e6cef3): for a two-level type the lower level gives way to the upper at the change point.
// The repair is accepted together with the level edit that caused it: one change, statistics reset once, and a SCPI
// query after the edit reads the ordered pair without a step.
#[test]
fn a_two_level_trigger_repair_is_accepted_with_the_level_that_caused_it() {
    let mut ui = Ui::for_tests(Source::Demo { t: 0.0 });
    ui.meas_add(Meas::Vpp, WaveId::Ch(0)).expect("a free row");
    let mut scpi = crate::scpi::Scpi::open(0u16, false);
    let cv = Canvas::new(1, 1);
    let mut q = |ui: &mut Ui, c: &str| String::from_utf8(scpi.exec_line(ui, &cv, c)).expect("SCPI text");
    q(&mut ui, ":TRIG:TYPE RUNT;:TRIG:RUNT:UPP 2;:TRIG:RUNT:LOW 1.6");
    for _ in 0..3 {
        ui.step(0.05);
    }
    assert!(ui.measurement_statistics_snapshot_for_test()[0].0 > 0);
    assert_eq!(q(&mut ui, ":TRIG:EDGE:LEV 0.4;:TRIG:RUNT:LOW?"), "4.000000E-01", "the lower level gives way");
    assert_eq!((ui.m.trig_level, ui.m.trig_level2), (0.4, 0.4));
    let accepted = ui.host.runtime().change.last.as_ref().expect("an accepted change");
    assert!(accepted.classes().contains(C::ACQ | C::STATS), "{:?}", accepted.classes());
    assert!(ui.measurement_statistics_snapshot_for_test().is_empty(), "the edit and its repair reset once");
}

// Review 178: a SCPI change point inside the program's intake is charged to the change, consumer and picture rings
// only. The timing rings get a supplied clock that advances one nanosecond per read, so the bound holds at any machine
// speed: rings that share no time add up to at most the number of clock reads.
#[test]
fn scpi_settlement_time_enters_one_ring_only() {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;
    let mut ui = Ui::for_tests(Source::Demo { t: 0.0 });
    ui.meas_add(Meas::Vpp, WaveId::Ch(0)).expect("a free row");
    for _ in 0..3 {
        ui.step(0.05);
    }
    assert!(ui.measurement_statistics_snapshot_for_test()[0].0 > 0);
    let reads = Arc::new(AtomicU64::new(0));
    let origin = std::time::Instant::now();
    let ticks = reads.clone();
    let clock = crate::perfguard::Clock::new(move || {
        origin + std::time::Duration::from_nanos(ticks.fetch_add(1, Ordering::Relaxed))
    });
    *change::timing_mut(&mut ui) = crate::perfguard::StageTiming::with_clock(clock);
    let mut scpi = crate::scpi::Scpi::open(0u16, false);
    let cv = Canvas::new(1, 1);
    let mut program = crate::scpi::Program::new(":MEAS:STAT:RES;:TRIG:STAT?;:TIM:SCAL 1e-3");
    assert!(scpi.poll_program(&mut ui, &cv, &mut program).is_some());
    let passed = reads.load(Ordering::Relaxed);
    let [intake, change, consumers, picture] = change::timing(&ui).attributed_ns();
    assert!(ui.measurement_statistics_snapshot_for_test().is_empty(), "the reset was accepted");
    assert!(intake > 0 && change > 0 && consumers > 0, "intake={intake} change={change} consumers={consumers}");
    assert!(
        intake + change + consumers + picture <= passed,
        "intake={intake} change={change} consumers={consumers} picture={picture} passed={passed}"
    );
}
