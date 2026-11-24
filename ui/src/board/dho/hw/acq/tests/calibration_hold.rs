// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::board::{CounterWant, DvmWant};
use oos_hal::dho::{counter, dvm, seq::Segments};

const VIEW_POLL: Duration = Duration::from_millis(20);
const CONFIGURE_RETRY: Duration = Duration::from_millis(500);
const SINGLE_POLL: Duration = Duration::from_millis(2);
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
fn stopped_view_after_calibration_services_dvm_and_levels_without_rereading() {
    let outcome = twice(|| calibrate_held(After::View));
    let held = after_calibration(&outcome.events);
    assert_eq!(trigger_plans(held), 1);
    assert!(!held.iter().any(|event| matches!(event, Event::Dma(_)) || is_arm(event)));
    let dvm_start = dvm::start(0, dvm::EXP)[0];
    assert!(held.contains(&Event::Write(dvm_start.0, dvm_start.1)), "the stopped view starved DVM");
    assert!(held.iter().any(|event| matches!(event, Event::Write(LEVELS, _))), "the stopped view starved levels");
    assert_eq!(held[held.len() - 2..], [Event::Wait(HELD_POLL), Event::Wait(HELD_POLL)]);
    assert_eq!(outcome.latest, Some(Frame::stamp(4, 3)));
}

#[test]
fn a_second_calibration_restores_settings_again_while_held() {
    let outcome = twice(|| calibrate_held(After::Again));
    let held: Vec<_> = outcome.events.split(|event| matches!(event, Event::Calibration(_))).collect();
    let [_, first, second] = held.as_slice() else { panic!("expected two calibrations") };
    let dvm_start = dvm::start(0, dvm::EXP)[0];
    for trace in [first, second] {
        assert_eq!(trigger_plans(trace), 1);
        let plan = trace.iter().position(is_trigger_plan).unwrap();
        let dvm = trace.iter().position(|event| *event == Event::Write(dvm_start.0, dvm_start.1)).unwrap();
        assert!(plan < dvm);
        assert!(!trace.iter().any(|event| matches!(event, Event::Dma(_)) || is_arm(event)));
    }
    assert_eq!(outcome.latest, Some(Frame::stamp(4, 3)));
}

#[test]
fn held_calibration_retries_a_configure_error_before_advancing_the_ack() {
    let outcome = twice(configure_error_after_calibration);
    let held = after_calibration(&outcome.events);
    let attempts: Vec<_> = held.split(|event| *event == Event::Wait(CONFIGURE_RETRY)).collect();
    let [first, second, restored] = attempts.as_slice() else { panic!("expected two configure retries") };
    let dvm_start = dvm::start(0, dvm::EXP)[0];
    let dvm_start = Event::Write(dvm_start.0, dvm_start.1);
    assert!(first.is_empty() && second.is_empty());
    assert_eq!(trigger_plans(restored), 1);
    assert!(restored.contains(&dvm_start));
    assert!(!held.iter().any(|event| matches!(event, Event::Dma(_)) || is_arm(event)));
    assert_eq!(outcome.final_ack, 8);
}

#[test]
fn calibration_during_a_completed_single_hold_preserves_the_capture() {
    let outcome = twice(calibrate_completed_single);
    let held = after_calibration(&outcome.events);
    assert_eq!(outcome.events.iter().filter(|event| is_arm(event)).count(), 1);
    assert_eq!(outcome.events.iter().filter(|event| matches!(event, Event::Dma(_))).count(), 1);
    assert_eq!(trigger_plans(held), 1);
    assert_eq!(outcome.latest, Some(Frame::stamp(1, 0)));
    assert_eq!(outcome.final_ack, 7);
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
    View,
    Again,
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
    let mut source = crate::wave::Source::Hw { shared: out.clone(), seen: 0 };
    drive(&out, fixture, move |clock, pause| match pause {
        Pause::Park if !stopped => {
            clock.stop();
            stopped = true;
            if after == After::View {
                assert!(source.hw_stop(Some((1e-6, 0.0))).is_none());
            }
        }
        Pause::Wait(HELD_POLL | VIEW_POLL) => {
            passes += 1;
            let mut g = lock(&shared);
            match passes {
                1 => {
                    if after == After::View {
                        assert!(g.replay.is_some(), "the first stopped view rereads the captured batch");
                    }
                    g.cal_req = Some((vec![0], true, false));
                    g.want.as_mut().unwrap().level = 0.5;
                    g.dvm_want = Some(DvmWant { ch: 0, lane: None });
                    if after == After::View {
                        g.slots_want = Some(1);
                    }
                }
                2 if after == After::View => {
                    assert!(g.replay.is_none());
                    drop(g);
                    assert!(source.hw_stop(Some((1e-6, 1e-6))).is_none());
                }
                2 if after == After::Run => {
                    drop(g);
                    clock.release();
                }
                2 if after == After::Again => {
                    g.cal_req = Some((vec![0], true, false));
                    g.want.as_mut().unwrap().level = -0.5;
                }
                _ => {
                    assert!(g.replay.is_none());
                    if after == After::View {
                        assert!(matches!(g.slots, Some((1, Some(_)))));
                    }
                    g.record_flow.shutdown = true;
                }
            }
        }
        Pause::Park => lock(&shared).record_flow.shutdown = true,
        Pause::Wait(delay) if delay == Duration::from_micros(1678) => clock.poke(LEVELS, 1 << 30),
        Pause::Wait(_) => {}
    })
}

fn configure_error_after_calibration() -> Outcome {
    let _scope = Scope::new();
    let want = settings();
    let out = requested(&want, Identity::default(), false);
    let fixture = sim::fixture(&want, out.clone(), (Run::Selection, 4, false), None);
    let shared = out.clone();
    let mut held = false;
    let mut failures = 0;
    drive(&out, fixture, move |clock, pause| match pause {
        Pause::Park => clock.stop(),
        Pause::Wait(HELD_POLL) => {
            let mut g = lock(&shared);
            if held {
                assert_eq!(g.final_ack, 8);
                assert!(g.replay.is_none());
                g.record_flow.shutdown = true;
            } else {
                assert_eq!(g.final_ack, 7);
                held = true;
                g.cal_req = Some((vec![0], true, false));
                g.final_req = 8;
                g.stop_view = Some((1e-6, 0.0));
                g.want.as_mut().unwrap().trig_src = 2;
                g.dvm_want = Some(DvmWant { ch: 0, lane: None });
            }
        }
        Pause::Wait(CONFIGURE_RETRY) => {
            let mut g = lock(&shared);
            assert_eq!(g.status.latest(), "settings: channel mask 1..0xf with the trigger source enabled");
            assert_eq!(g.final_ack, 7);
            assert!(g.replay.is_none());
            failures += 1;
            if failures == 2 {
                g.want.as_mut().unwrap().trig_src = want.trig_src;
            }
        }
        Pause::Wait(_) => {}
    })
}

fn calibrate_completed_single() -> Outcome {
    let _scope = Scope::new();
    let want = Settings { auto: false, single: true, ..settings() };
    let out = requested(&want, Identity::default(), false);
    let fixture = sim::fixture(&want, out.clone(), (Run::Selection, 1, false), None);
    let shared = out.clone();
    let mut calibrated = false;
    drive(&out, fixture, move |clock, pause| match pause {
        Pause::Wait(SINGLE_POLL) => {
            assert_eq!(lock(&shared).queue.len(), 1);
            clock.stop();
        }
        Pause::Wait(HELD_POLL) => {
            let mut g = lock(&shared);
            assert_eq!(g.final_ack, 7);
            if calibrated {
                assert!(g.replay.is_none());
                g.record_flow.shutdown = true;
            } else {
                calibrated = true;
                g.cal_req = Some((vec![0], true, false));
                g.stop_view = Some((1e-6, 0.0));
            }
        }
        Pause::Park => panic!("a completed SINGLE waits for the UI hold"),
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
