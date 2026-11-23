// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::model::{Model, TrigMode};
use oos_core::change::{Request as ChangeRequest, Shadow, UiAcq};

#[test]
fn held_pass_without_a_view_writes_no_registers_for_any_request() {
    for levels in [&[][..], &[0.5][..]] {
        let outcome = twice(|| stopped_pass(levels, Resume::No));
        let stop = outcome.events.iter().position(|event| *event == Event::Stop).unwrap();
        assert_eq!(outcome.events[stop + 1..], [Event::Wait(HELD_POLL)]);
        assert_eq!(outcome.final_ack, 7);
        assert_eq!(outcome.latest, Some(Frame::stamp(4, 3)));
    }
}

#[test]
fn run_applies_only_the_latest_held_request_before_the_first_arm() {
    for levels in [&[0.5][..], &[0.25, 0.5][..]] {
        let outcome = twice(|| stopped_pass(levels, Resume::Run));
        let stop = outcome.events.iter().position(|event| *event == Event::Stop).unwrap();
        let wake = outcome.events.iter().position(|event| *event == Event::SourceWake).unwrap();
        let held = &outcome.events[stop + 1..wake];
        assert_eq!(held.len(), levels.len());
        assert!(held.iter().all(|event| *event == Event::Wait(HELD_POLL)), "{held:?}");
        let run = &outcome.events[wake + 1..];
        let arms: Vec<_> = run.iter().enumerate().filter(|(_, event)| is_arm(event)).map(|(index, _)| index).collect();
        assert_eq!(arms.len(), 2);
        let expected = [Event::Write(0x2004, 0x85dc_8ea6), Event::Write(0x2008, 0x8ea6_9770)];
        let level_writes: Vec<_> =
            run.iter().filter(|event| matches!(event, Event::Write(0x2004 | 0x2008, _))).collect();
        assert_eq!(level_writes, expected.each_ref());
        assert!(run[..arms[0]].windows(expected.len()).any(|writes| writes == expected));
    }
}

#[test]
fn reverted_held_edit_needs_no_configuration_on_run() {
    let outcome = twice(|| stopped_pass(&[0.5, 0.0], Resume::Run));
    let stop = outcome.events.iter().position(|event| *event == Event::Stop).unwrap();
    assert_eq!(trigger_plans(&outcome.events[stop..]), 0);
    assert_eq!(outcome.events[stop + 1..stop + 3], [Event::Wait(HELD_POLL), Event::Wait(HELD_POLL)]);
    assert_eq!(outcome.events[stop..].iter().filter(|event| is_arm(event)).count(), 2);
}

#[test]
fn single_applies_the_held_edit_before_arming_once() {
    let outcome = twice(|| stopped_pass(&[0.5], Resume::Single));
    let stop = outcome.events.iter().position(|event| *event == Event::Stop).unwrap();
    let wake = outcome.events.iter().position(|event| *event == Event::SourceWake).unwrap();
    assert_eq!(outcome.events[stop + 1..wake], [Event::Wait(HELD_POLL)]);
    let single = &outcome.events[wake + 1..];
    let arm = single.iter().position(is_arm).unwrap();
    assert_eq!(trigger_plans(&single[..arm]), 1);
    assert!(single[..arm].contains(&Event::Write(0x2004, 0x85dc_8ea6)));
    assert_eq!(single.iter().filter(|event| is_arm(event)).count(), 1);
    assert_eq!(single.iter().filter(|event| matches!(event, Event::Dma(_))).count(), 1);
    assert_eq!(single.last(), Some(&Event::Wait(Duration::from_millis(2))));
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Resume {
    No,
    Run,
    Single,
}

fn stopped_pass(levels: &'static [f32], resume: Resume) -> Outcome {
    let _scope = Scope::new();
    let want = settings();
    let out = requested(&want, Identity::default(), false);
    let fixture = sim::fixture(&want, out.clone(), (Run::Selection, 4, false), None);
    let shared = out.clone();
    let mut levels = levels.iter().copied();
    let mut stopped = false;
    let mut resumed_batches = 0;
    let mut publish = held_edits();
    drive(&out, fixture, move |clock, pause| match pause {
        Pause::Park if !stopped => {
            clock.stop();
            stopped = true;
            let mut g = lock(&shared);
            assert!(g.stop_view.is_none());
            if let Some(level) = levels.next() {
                publish(&mut g, Some(level), false);
            }
        }
        Pause::Wait(HELD_POLL) => {
            if let Some(level) = levels.next() {
                publish(&mut lock(&shared), Some(level), false);
            } else if resume != Resume::No {
                clock.release();
                publish(&mut lock(&shared), None, resume == Resume::Single);
            } else {
                lock(&shared).record_flow.shutdown = true;
            }
        }
        Pause::Wait(delay) if resume == Resume::Single && delay == Duration::from_millis(2) => {
            lock(&shared).record_flow.shutdown = true;
        }
        Pause::Park => {
            resumed_batches += 1;
            let mut g = lock(&shared);
            assert!(!g.hold);
            assert_eq!(g.queue.len(), 4);
            g.queue.clear();
            g.record_flow.drained();
            g.record_flow.shutdown = resumed_batches == 2;
        }
        Pause::Wait(_) => {}
    })
}

fn held_edits() -> impl FnMut(&mut crate::board::Shared, Option<f32>, bool) {
    let config = crate::model::config();
    let mut model = Model { trig_level: 0.0, ..Model::default() };
    let mut shadow = Shadow::new(&model, UiAcq::default(), &config).unwrap();
    move |g, level, single| {
        let want = g.want.as_mut().unwrap();
        if let Some(level) = level {
            (model.trig_level, want.level) = (level, f64::from(level));
        }
        let requests = if level.is_none() { &[ChangeRequest::Run][..] } else { &[] };
        if single {
            model.trig_mode = TrigMode::Single;
            (want.auto, want.single) = (false, true);
        }
        let id = shadow.observe(&model, UiAcq::default(), requests, &config).unwrap().identity();
        assert_ne!(id, g.acquisition.identity());
        g.acquisition = crate::board::identity::Publication::new(id, false);
        g.record_flow.wake();
    }
}
