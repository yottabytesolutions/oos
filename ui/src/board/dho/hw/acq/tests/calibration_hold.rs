// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::board::{CounterWant, DvmWant};
use oos_hal::dho::{counter, dvm, seq::Segments};

const LEVELS: u32 = 0x1040;

#[test]
fn held_calibration_restores_one_plan_before_dvm_and_discards_replay() {
    let outcome = twice(|| calibrate_held(After::Hold));
    let held = after_calibration(&outcome.events);
    assert_eq!(trigger_plans(held), 1);
    let configured = held.iter().position(|event| *event == Event::Write(0x2004, 0x85dc_8ea6)).unwrap();
    let dvm: Vec<_> = dvm::start(0, dvm::EXP).into_iter().map(|(reg, val)| Event::Write(reg, val)).collect();
    assert!(held[configured + 1..].windows(dvm.len()).any(|writes| writes == dvm));
    assert!(!held.iter().any(|event| matches!(event, Event::Dma(_)) || is_arm(event)));
    assert_eq!(outcome.latest, Some(Frame::stamp(4, 3)));
}

#[test]
fn run_after_held_calibration_does_not_apply_the_restored_plan_again() {
    let outcome = twice(|| calibrate_held(After::Run));
    let trace = after_calibration(&outcome.events);
    let wake = trace.iter().position(|event| *event == Event::SourceWake).unwrap();
    assert_eq!(trigger_plans(&trace[..wake]), 1);
    assert_eq!(trigger_plans(&trace[wake..]), 0);
    assert!(trace[wake..].iter().any(is_arm));
}

#[test]
fn unconfigured_hold_restores_settings_before_side_units_and_never_arms() {
    let outcome = twice(|| start_held(Slow::Off, false));
    assert_eq!(trigger_plans(&outcome.events), 1);
    let plan = outcome.events.iter().position(is_trigger_plan).unwrap();
    let counter = outcome.events.iter().position(|event| matches!(event, Event::Write(counter::CTRL, _))).unwrap();
    let dvm_start = dvm::start(0, dvm::EXP)[0];
    let dvm = outcome.events.iter().position(|event| *event == Event::Write(dvm_start.0, dvm_start.1)).unwrap();
    let levels = outcome.events.iter().position(|event| matches!(event, Event::Write(LEVELS, _))).unwrap();
    assert!(plan < counter && plan < dvm && plan < levels);
    assert!(outcome.latest.is_none());
}

#[test]
fn roll_and_scan_hold_poll_without_advancing_the_initial_acknowledgement() {
    for mode in [Slow::Roll, Slow::Scan { delay: 0.0, single: true }] {
        let outcome = twice(|| start_held(mode, false));
        assert_eq!(outcome.final_ack, 7);
        assert_eq!(trigger_plans(&outcome.events), 1);
    }
}

#[test]
fn segments_hold_polls_every_five_milliseconds_without_arming() {
    let outcome = twice(|| start_held(Slow::Off, true));
    assert_eq!(outcome.events[outcome.events.len() - 2..], [Event::Wait(HELD_POLL), Event::Wait(HELD_POLL)]);
    assert_eq!(trigger_plans(&outcome.events), 1);
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum After {
    Hold,
    Run,
}

fn after_calibration(events: &[Event]) -> &[Event] {
    let cal = events.iter().position(|event| matches!(event, Event::Calibration(_))).unwrap();
    &events[cal + 1..]
}

fn calibrate_held(after: After) -> Outcome {
    let _scope = Scope::new();
    let want = Settings { tdiv: 10e-6, depth: 14_000, ..settings() };
    let out = requested(&want, Identity::default(), false);
    let fixture = sim::fixture(&want, out.clone(), (Run::Selection, 4, false), None);
    let shared = out.clone();
    let mut stopped = false;
    let mut passes = 0;
    drive(&out, fixture, move |clock, pause| match pause {
        Pause::Park if !stopped => {
            clock.stop();
            stopped = true;
        }
        Pause::Wait(HELD_POLL) => {
            passes += 1;
            let mut g = lock(&shared);
            match passes {
                1 => {
                    g.cal_req = Some((vec![0], true, false));
                    g.want.as_mut().unwrap().level = 0.5;
                    g.dvm_want = Some(DvmWant { ch: 0, lane: None });
                }
                2 if after == After::Run => {
                    drop(g);
                    clock.release();
                }
                _ => {
                    assert!(g.replay.is_none());
                    g.record_flow.shutdown = true;
                }
            }
        }
        Pause::Park => lock(&shared).record_flow.shutdown = true,
        Pause::Wait(_) => {}
    })
}

fn start_held(mode: Slow, segments: bool) -> Outcome {
    let _scope = Scope::new();
    let want = Settings { segments: segments.then_some(Segments { n: 4, interval: 0.0 }), ..settings() };
    let out = requested(&want, Identity::default(), false);
    {
        let mut g = lock(&out);
        (g.hold, g.final_req, g.slow) = (true, 7, mode);
        if mode == Slow::Off && !segments {
            g.counter_want = Some(CounterWant { ch: 0, mode: 0, digits: 3, clear: 0 });
            g.dvm_want = Some(DvmWant { ch: 0, lane: None });
            g.slots_want = Some(1);
        }
    }
    let fixture = sim::fixture(&want, out.clone(), (Run::Selection, 4, false), None);
    let shared = out.clone();
    let mut passes = 0;
    let outcome = drive(&out, fixture, move |clock, pause| match pause {
        Pause::Wait(HELD_POLL) => {
            passes += 1;
            let mut g = lock(&shared);
            g.final_req = 8;
            if passes == 2 {
                g.record_flow.shutdown = true;
            }
        }
        Pause::Wait(delay) if delay == Duration::from_micros(1678) => clock.poke(LEVELS, 1 << 30),
        _ => panic!("unexpected held-mode wait: {pause:?}"),
    });
    assert!(!outcome.events.iter().any(|event| matches!(event, Event::Dma(_)) || is_arm(event)));
    if mode == Slow::Off && !segments {
        assert!(matches!(lock(&out).slots, Some((1, Some(_)))));
    }
    outcome
}
