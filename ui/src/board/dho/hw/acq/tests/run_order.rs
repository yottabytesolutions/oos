// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::{
    model::{RunState, TrigMode},
    ui::{change, key, Ui},
    wave::Source,
};

#[test]
fn run_publishes_changed_settings_before_waking_the_held_source() {
    let result = twice(|| resume(true));
    assert_order(&result, true);
}

#[test]
fn run_publishes_a_new_epoch_even_when_settings_are_unchanged() {
    let result = twice(|| resume(false));
    assert_order(&result, false);
}

fn assert_order(result: &Outcome, changed: bool) {
    let events = &result.events;
    let publish = events.iter().position(|e| *e == Event::SettingsPublished).unwrap();
    let wake = events.iter().position(|e| *e == Event::SourceWake).unwrap();
    let applied = events.iter().position(|e| *e == Event::SettingsApplied);
    let arm = events.iter().position(is_arm).unwrap();
    let dma = events.iter().position(|e| matches!(e, Event::Dma(_))).unwrap();
    let intake = events.iter().position(|e| matches!(e, Event::Intake(_))).unwrap();
    assert!(publish < wake && wake < arm && arm < dma && dma < intake);
    if changed {
        let applied = applied.unwrap();
        assert!(wake < applied && applied < arm);
    } else {
        assert!(applied.is_none(), "an equal request reuses the applied settings");
        assert!(events[..publish].iter().any(is_trigger_plan));
    }
    assert_eq!(result.counters.deposited, 4);
}

pub(super) fn headless(out: &HwShared, stopped: bool) -> Ui {
    let mut ui = Ui::for_tests(Source::Hw { shared: out.clone(), seen: 0 });
    ui.m = picture::model(settings().tdiv);
    ui.m.trig_mode = TrigMode::Normal;
    ui.m.run = if stopped { RunState::Stop } else { RunState::Run };
    if stopped {
        ui.key(key::CLEAR);
    }
    change::settle(&mut ui);
    assert_eq!(lock(out).hold, stopped);
    ui
}

fn accept(out: &HwShared, commit: impl FnOnce()) -> [Event; 2] {
    let before = lock(out).record_flow.command_generations();
    commit();
    let g = lock(out);
    let (identity, at_commit) = g.record_flow.acquisition_commit.unwrap();
    assert_eq!(identity, g.acquisition.identity());
    assert_eq!(at_commit.1, before.1, "pipe source and handoff.commands woke before commit");
    assert_eq!(
        g.record_flow.command_generations(),
        (before.0 + 1, before.1.map(|(source, commands)| (source + 1, commands + 1))),
    );
    // The commit snapshot places publication before or after the actual source notification.
    if at_commit.0 == before.0 {
        [Event::SettingsPublished, Event::SourceWake]
    } else {
        [Event::SourceWake, Event::SettingsPublished]
    }
}

fn resume(changed: bool) -> Outcome {
    let scope = Scope::new();
    let intake = scope.gated_worker();
    let out = HwShared::default();
    let mut ui = headless(&out, true);
    let (want, old) = {
        let g = lock(&out);
        (g.want.clone().unwrap(), g.acquisition.identity())
    };
    {
        let mut g = lock(&out);
        g.stop_view = Some(StopView { capture: 0, scale: want.tdiv, delay: 0.0 });
        g.inject_stop_reply(crate::board::Frame::default());
        g.queue.push_back(crate::board::Frame { cfg: old.gen(), stamp: old.epoch(), ..Default::default() }.into());
    }
    let run = Run::Pictures(picture::StopAt::Between);
    let fixture = sim::fixture(&want, out.clone(), (run, 4, false), Some(intake));
    let shared = out.clone();
    drive(&out, fixture, move |clock, pause| match pause {
        Pause::Wait(HELD_POLL) if lock(&shared).hold => {
            ui.m.run = RunState::Run;
            if changed {
                ui.m.trig_level = 0.5;
            }
            let events = accept(&shared, || change::settle(&mut ui));
            let g = lock(&shared);
            assert_ne!(g.acquisition.identity().epoch(), old.epoch());
            assert_eq!(g.want.as_ref().unwrap().level, if changed { 0.5 } else { want.level });
            assert_eq!(g.want.as_ref() == Some(&want), !changed);
            assert!(!g.hold);
            assert!(g.stop_view.is_none() && g.stop_replies.frame.is_none() && g.queue.is_empty());
            for event in events {
                clock.mark(event);
            }
        }
        Pause::Park => {
            assert!(clock.take_submission());
            lock(&shared).record_flow.shutdown = true;
        }
        Pause::Wait(_) => {}
    })
}

#[test]
fn settings_change_during_run_retires_the_previous_intake() {
    let result = twice(|| change_during_run(false));
    let events = &result.events;
    let publish = events.iter().position(|e| *e == Event::SettingsPublished).unwrap();
    let intake = events.iter().position(|e| matches!(e, Event::Intake(_))).unwrap();
    assert!(publish < intake);
    assert_eq!(result.counters.loss("lost.settings"), 4, "{:?}", result.counters);
    assert_eq!(result.counters.deposited, 4);
}

#[test]
fn settings_change_during_dma_accounts_for_old_records_before_intake() {
    let result = twice(|| change_during_run(true));
    let events = &result.events;
    let publish = events.iter().position(|e| *e == Event::SettingsPublished).unwrap();
    let applied = events.iter().position(|e| *e == Event::SettingsApplied).unwrap();
    let dma: Vec<_> = events.iter().enumerate().filter(|(_, e)| matches!(e, Event::Dma(_))).map(|(i, _)| i).collect();
    let intake = events.iter().position(|e| matches!(e, Event::Intake(_))).unwrap();
    assert_eq!(dma.len(), 2, "{:?} {:?}", dma.iter().map(|i| &events[*i]).collect::<Vec<_>>(), result.counters);
    assert!(publish < dma[0] && dma[0] < applied && applied < dma[1] && dma[1] < intake);
    assert_eq!(result.counters.read, 8);
    assert_eq!(result.counters.loss("lost.settings"), 4);
    assert_eq!(result.counters.deposited, 4);
}

#[test]
fn settings_change_during_average_dma_rejects_the_old_result() {
    let result = twice(|| {
        let _scope = Scope::new();
        let out = HwShared::default();
        let mut ui = headless(&out, false);
        ui.m.acq = crate::model::AcqMode::Average;
        ui.m.avg_hw = true;
        ui.m.avg_count = 2;
        change::settle(&mut ui);
        let want = lock(&out).want.clone().unwrap();
        assert_eq!(want.mode, Mode::Average { count: 2 });
        let (mut io, trace) = sim::fixture(&want, out.clone(), (Run::Selection, 2, false), None);
        let (mut source, started) = source(&out, &io);
        source.switches.board.frame0_drop = true;
        let shared = out.clone();
        io.c2h.on_dma(move |trace| trace.events.extend(edit_level(&mut ui, &shared)));
        capture(&mut source, &mut io);
        snapshot(&out, &io, &trace, started)
    });
    assert!(result.queue.is_empty() && result.latest.is_none());
    assert_eq!(result.counters.read, 2);
    assert_eq!(result.counters.loss("lost.settings") + result.counters.loss("lost.average"), 2);
    assert!(result.counters.loss("lost.settings") > 0);
    assert_eq!(result.counters.loss("lost.frame1"), 0);
}

#[test]
fn settings_change_during_overlapping_fill_cancels_old_dma_admission() {
    let result = twice(change_during_fill);
    let events = &result.events;
    let publish = events.iter().position(|e| *e == Event::SettingsPublished).unwrap();
    let applied = events.iter().position(|e| *e == Event::SettingsApplied).unwrap();
    let dma = events.iter().position(|e| matches!(e, Event::Dma(_))).unwrap();
    assert!(publish < applied && applied < dma);
    assert_eq!(result.counters.read, 4);
    assert_eq!(result.counters.deposited, 4);
}

fn change_during_fill() -> Outcome {
    let scope = Scope::new();
    let intake = scope.gated_worker();
    let out = HwShared::default();
    let mut ui = headless(&out, false);
    let want = lock(&out).want.clone().unwrap();
    let run = Run::Pictures(picture::StopAt::Between);
    let (mut io, trace) = sim::fixture(&want, out.clone(), (run, 4, false), Some(intake));
    let (mut source, started) = source(&out, &io);
    let shared = out.clone();
    let mut committed = false;
    io.clock.drive(move |clock, pause| {
        if !committed && pause == Pause::Wait(Duration::from_micros(200)) && clock.capture_armed() {
            assert!(!lock(&shared).record_flow.serial);
            for event in edit_level(&mut ui, &shared) {
                clock.mark(event);
            }
            committed = true;
        }
    });
    capture(&mut source, &mut io);
    assert!(!io.clock.take_submission());
    capture(&mut source, &mut io);
    assert!(io.clock.take_submission());
    snapshot(&out, &io, &trace, started)
}

pub(super) fn edit_level(ui: &mut Ui, out: &HwShared) -> [Event; 2] {
    let old = lock(out).acquisition.identity();
    ui.m.trig_level = 0.5;
    let events = accept(out, || change::settle(ui));
    let g = lock(out);
    assert!(!g.hold);
    assert_ne!(g.acquisition.identity().gen(), old.gen());
    assert_eq!(g.want.as_ref().unwrap().level, 0.5);
    events
}

fn change_during_run(during_dma: bool) -> Outcome {
    let scope = Scope::new();
    let intake = scope.gated_worker();
    let out = HwShared::default();
    let mut ui = headless(&out, false);
    let want = lock(&out).want.clone().unwrap();
    let run = Run::Pictures(picture::StopAt::Between);
    let (mut io, trace) = sim::fixture(&want, out.clone(), (run, 4, false), Some(intake));
    let (mut source, started) = source(&out, &io);
    if during_dma {
        let shared = out.clone();
        io.c2h.on_dma(move |trace| {
            trace.events.extend(edit_level(&mut ui, &shared));
        });
        capture(&mut source, &mut io);
        assert!(!io.clock.take_submission());
        assert!(lock(&out).trig_fired.is_none(), "late DMA cannot publish status for the new pair");
    } else {
        capture(&mut source, &mut io);
        for event in edit_level(&mut ui, &out) {
            io.clock.mark(event);
        }
        assert!(io.clock.take_submission());
    }
    assert_eq!(RecordCounts::read().deposited, 0);
    capture(&mut source, &mut io);
    assert!(io.clock.take_submission());
    snapshot(&out, &io, &trace, started)
}

#[test]
fn single_from_stop_commits_before_arming_the_new_pair() {
    let result = twice(single_from_stop);
    let events = &result.events;
    let publish = events.iter().position(|e| *e == Event::SettingsPublished).unwrap();
    let applied = events.iter().position(|e| *e == Event::SettingsApplied).unwrap();
    let arm = events.iter().position(is_arm).unwrap();
    let dma = events.iter().position(|e| matches!(e, Event::Dma(_))).unwrap();
    assert_eq!(events[publish + 1], Event::SourceWake);
    assert!(publish + 1 < applied && applied < arm && arm < dma);
    assert_eq!(result.queue.len(), 1);
    assert_eq!(result.counters.read, 1);
}

fn single_from_stop() -> Outcome {
    let _scope = Scope::new();
    let out = HwShared::default();
    let mut ui = headless(&out, true);
    let want = lock(&out).want.clone().unwrap();
    // Start the pipe before observing generations, as in RUN after the first live pass.
    drawpipe::start_pipe(&Switches::default());
    let fixture = sim::fixture(&want, out.clone(), (Run::Selection, 1, false), None);
    let shared = out.clone();
    drive(&out, fixture, move |clock, pause| match pause {
        Pause::Wait(HELD_POLL) => {
            let previous = lock(&shared).acquisition.identity();
            let events = accept(&shared, || {
                let mut scpi = crate::scpi::Scpi::disconnected();
                scpi.exec_line_checked(&mut ui, &crate::Canvas::new(1, 1), ":SINGle").unwrap();
            });
            let g = lock(&shared);
            assert!(!g.hold && !g.acquisition.route());
            assert!(g.want.as_ref().unwrap().single);
            assert_ne!(g.acquisition.identity().epoch(), previous.epoch());
            for event in events {
                clock.mark(event);
            }
        }
        Pause::Wait(delay) if delay == Duration::from_millis(2) => lock(&shared).record_flow.shutdown = true,
        _ => {}
    })
}

#[test]
fn observer_rejection_leaves_publication_and_hold_in_force() {
    for held in [false, true] {
        twice(|| rejected(held));
    }
}

fn rejected(held: bool) -> Outcome {
    let _scope = Scope::new();
    let out = HwShared::default();
    let mut ui = headless(&out, held);
    if !held {
        ui.m.run = RunState::Stop;
        change::settle(&mut ui); // Bootstrap without a first record still permits acquisition.
    }
    let before = lock(&out).record_flow.command_generations();
    let g = lock(&out);
    assert_eq!(g.hold, held);
    let (want, publication) = (g.want.clone().unwrap(), g.acquisition);
    drop(g);
    let fixture = sim::fixture(&want, out.clone(), (Run::Selection, 4, false), None);
    let shared = out.clone();
    drive(&out, fixture, move |_, pause| {
        if (held && pause != Pause::Wait(HELD_POLL)) || (!held && pause != Pause::Park) {
            return;
        }
        ui.m.meas.reserve_exact(64 * 1024); // Exceeds the bounded observer's snapshot capacity.
        if held {
            ui.m.run = RunState::Run;
            ui.m.trig_level = 0.5;
        } else {
            ui.key(key::CLEAR);
        }
        change::settle(&mut ui);
        assert_eq!(lock(&shared).record_flow.command_generations(), before);
        let mut g = lock(&shared);
        assert_eq!(g.hold, held);
        assert_eq!(g.acquisition.identity(), publication.identity());
        assert_eq!(g.acquisition.route(), publication.route());
        assert_eq!(g.want.as_ref(), Some(&want));
        g.record_flow.shutdown = true;
    })
}

#[test]
fn segmented_restart_commits_new_settings_with_its_command() {
    let result = twice(segmented_restart);
    let events = &result.events;
    let publish = events.iter().position(|e| *e == Event::SettingsPublished).unwrap();
    let applied = events.iter().position(|e| *e == Event::SettingsApplied).unwrap();
    let arms: Vec<_> = events.iter().enumerate().filter(|(_, e)| is_arm(e)).map(|(i, _)| i).collect();
    assert_eq!(arms.len(), 2);
    assert_eq!(events[publish + 1], Event::SourceWake);
    assert!(arms[0] < publish && publish + 1 < applied && applied < arms[1]);
}

fn segmented_restart() -> Outcome {
    let _scope = Scope::new();
    let out = HwShared::default();
    let mut ui = headless(&out, false);
    ui.m.seg_hw = true;
    ui.m.segments = 4;
    change::settle(&mut ui);
    let want = lock(&out).want.clone().unwrap();
    assert!(want.segments.is_some());
    let (mut io, trace) = sim::fixture(&want, out.clone(), (Run::Selection, 4, false), None);
    let (mut source, started) = source(&out, &io);
    let old = segment_pass(&mut source, &mut io);
    io.clock.record_triggers(4);
    for event in edit_level(&mut ui, &out) {
        io.clock.mark(event);
    }
    assert_ne!(lock(&out).seg.run, old);
    assert_ne!(segment_pass(&mut source, &mut io), old);
    snapshot(&out, &io, &trace, started)
}

fn segment_pass(source: &mut AcqLoop<FixedPacing>, io: &mut Io<sim::Sim>) -> u64 {
    let request = source.service(io, None).unwrap();
    let (want, params) = source.reconfigure(io, &request, &mut false).unwrap();
    source.segments(io, &want, &params, request.identity);
    let g = lock(&source.out);
    assert!(g.seg.running);
    assert_eq!(g.seg.phase, crate::board::SegPhase::Recording);
    assert_eq!(g.seg.done_run, g.seg.run);
    g.seg.run
}
