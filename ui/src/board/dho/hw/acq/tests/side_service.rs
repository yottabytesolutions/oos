// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::board::{CounterWant, DvmRead, DvmWant, SlotDump};
use oos_hal::dho::{afg, board, counter, dvm};

#[test]
fn held_source_services_calibration_generator_counter_and_side_units() {
    let (outcome, xadc) = twice(service_while_held);
    let trace = &outcome.events;
    let stop = trace.iter().position(|event| *event == Event::Stop).unwrap();
    let held = &trace[stop + 1..];
    let config = counter::Config { ch: 0, mode: counter::Mode::Frequency, digits: 3 };
    let counter_writes: Vec<_> = counter::start(&config).into_iter().map(|(r, v)| Event::Write(r, v)).collect();
    let calibrate = held.iter().position(|event| *event == Event::Calibration((vec![0], true, false))).unwrap();
    let generator = held.iter().position(|event| *event == Event::Generator(Some(generator_want()))).unwrap();
    assert!(calibrate < generator);
    assert!(held[generator + 1..].windows(counter_writes.len()).any(|writes| writes == counter_writes));
    assert!(held.contains(&Event::Write(counter::CTRL, 0)));
    let dvm: Vec<_> = dvm::start(0, dvm::EXP).into_iter().map(|(r, v)| Event::Write(r, v)).collect();
    assert!(held[generator + 1..].windows(dvm.len()).any(|writes| writes == dvm));
    assert!(held.contains(&Event::Read(board::XADC_REG, XADC)));
    assert_eq!(xadc, Some(board::xadc(XADC)));
    assert!(!held.iter().any(|event| matches!(event, Event::Dma(_))));
    assert_eq!(outcome.final_ack, 7);
}

const XADC: u32 = (1365 << 16) | 2424;
const VIEW_POLL: Duration = Duration::from_millis(20);
const LEVELS: u32 = 0x1040;
const SLOT_WAIT: Duration = Duration::from_micros(1678);

fn generator_want() -> crate::board::Gen {
    (true, 0.001, 1.0, 0.0, afg::Wave::Sine, afg::Mod::None)
}

fn service_while_held() -> (Outcome, Option<(f32, f32)>) {
    let _scope = Scope::new();
    let want = settings();
    let out = requested(&want, Identity::default(), false);
    let fixture = sim::fixture(&want, out.clone(), (Run::Selection, 4, false), None);
    let shared = out.clone();
    let mut passes = 0;
    let outcome = drive(&out, fixture, move |clock, pause| match pause {
        Pause::Park => {
            clock.stop();
            clock.poke(board::XADC_REG, XADC);
            let mut g = lock(&shared);
            g.cal_req = Some((vec![0], true, false));
            g.afg[0] = Some(generator_want());
        }
        Pause::Wait(delay) if delay == Duration::from_millis(5) => {
            passes += 1;
            let mut g = lock(&shared);
            match passes {
                1 => {
                    g.counter_want = Some(CounterWant { ch: 0, mode: 0, digits: 3, clear: 0 });
                    g.dvm_want = Some(DvmWant { ch: 0, lane: None });
                }
                2 => {
                    g.counter_want = None;
                    g.dvm_want = None;
                }
                201 => g.record_flow.shutdown = true,
                _ => {}
            }
        }
        Pause::Wait(_) => {}
    });
    let g = lock(&out);
    assert!(g.cal_req.is_none());
    (outcome, g.xadc)
}

#[test]
fn stopped_view_services_dvm_with_the_applied_channels_and_scale() {
    let (outcome, reading, _) = twice(|| stopped_view_with(Side::Dvm));
    let stop = outcome.events.iter().position(|e| *e == Event::Stop).unwrap();
    let held = &outcome.events[stop + 1..];
    let start = dvm::start(2, dvm::EXP)[0];
    let dvm_start = held.iter().position(|e| *e == Event::Write(start.0, start.1)).expect("DVM must start in STOP");
    let dvm_read = held.iter().position(|e| *e == Event::Write(dvm::LATCH, 1)).expect("DVM must read in STOP");
    assert!(dvm_start < dvm_read);
    assert_eq!(held[dvm_read..].last(), Some(&Event::Wait(VIEW_POLL)));
    assert_eq!(held.iter().filter(|e| matches!(e, Event::Dma(_))).count(), 1);
    assert_eq!(trigger_plans(held), 0);
    let reading = reading.unwrap();
    assert_eq!((reading.ch, reading.lane, reading.n), (0, 2, 1));
    let [dc, ac, ac_dc] = reading.volts.unwrap();
    assert_eq!(dc, 100.0 * (16.0 * 0.2 / 7500.0));
    assert_eq!(ac, 0.0);
    assert!((dc - ac_dc).abs() < f64::EPSILON);
}

#[test]
fn stopped_view_services_levels_before_the_changed_window_and_then_waits() {
    let (outcome, _, slots) = twice(|| stopped_view_with(Side::Levels));
    let stop = outcome.events.iter().position(|e| *e == Event::Stop).unwrap();
    let held = &outcome.events[stop + 1..];
    let mut reads = held.iter().enumerate().filter(|(_, e)| matches!(e, Event::Dma(_)));
    let first = reads.next().unwrap().0;
    let second = reads.next().unwrap().0;
    assert!(reads.next().is_none());
    let levels = held.iter().position(|e| *e == Event::Wait(SLOT_WAIT)).expect("levels must run in STOP");
    assert!(first < levels && levels < second);
    assert_eq!(held.last(), Some(&Event::Wait(VIEW_POLL)));
    assert_eq!(trigger_plans(held), 0);
    assert_eq!(slots, Some((1, Some([[1000.0, 3000.0, 32000.0]; 8]))));
}

#[derive(Clone, Copy)]
enum Side {
    Dvm,
    Levels,
}

fn stopped_view_with(side: Side) -> (Outcome, Option<DvmRead>, Option<SlotDump>) {
    let _scope = Scope::new();
    let mut want = Settings { tdiv: 10e-6, depth: 14_000, ..settings() };
    want.ch[0].scale = 0.2;
    let out = requested(&want, Identity::default(), false);
    let fixture = sim::fixture(&want, out.clone(), (Run::Selection, 4, false), None);
    let shared = out.clone();
    let mut passes = 0;
    let outcome = drive(&out, fixture, move |clock, pause| match pause {
        Pause::Park => {
            clock.stop();
            lock(&shared).stop_view = Some(StopView { capture: 0, scale: 1e-6, delay: 0.0 });
        }
        Pause::Wait(VIEW_POLL) => {
            passes += 1;
            let mut g = lock(&shared);
            assert!(g.stop_replies.frame.is_some());
            match (passes, side) {
                (1, Side::Dvm) => {
                    let want = g.want.as_mut().unwrap();
                    want.mask = 3;
                    want.ch[0].scale = 0.5;
                    g.dvm_want = Some(DvmWant { ch: 0, lane: Some(2) });
                }
                (1, Side::Levels) => {
                    g.slots_want = Some(1);
                    g.stop_view = Some(StopView { capture: 0, scale: 1e-6, delay: 1e-6 });
                }
                (2, Side::Dvm) => {
                    let samples = 1_u64 << dvm::EXP;
                    let mean = 2148;
                    let (sum, square) = (samples * mean, samples * mean * mean);
                    clock.poke(dvm::SUM_LO, sum as u32);
                    clock.poke(dvm::SUM_HI, ((sum >> 32) as u32) << 16);
                    clock.poke(dvm::SQ_LO, square as u32);
                    clock.poke(dvm::SQ_HI, (square >> 32) as u32);
                    clock.advance(Duration::from_millis(500));
                }
                _ => g.record_flow.shutdown = true,
            }
        }
        Pause::Wait(SLOT_WAIT) => {
            clock.poke(LEVELS, 1 << 30);
            clock.poke(0x1044, 32000);
            clock.poke(0x1048, (3000 << 16) | 1000);
        }
        Pause::Wait(_) => {}
    });
    let g = lock(&out);
    (outcome, g.dvm, g.slots)
}

#[test]
fn unapplied_stop_edits_cannot_set_or_clear_the_mask_unit_verdict() {
    let _map = crate::board::dho::hw::mask_map_test_lock();
    for initially_blocked in [false, true] {
        let (outcome, status) = twice(|| edit_mask_unit_while_held(initially_blocked));
        let expected = [initially_blocked, initially_blocked, !initially_blocked];
        assert_eq!(status.each_ref().map(Option::is_some), expected);
        assert_eq!(status[0], status[1]);
        let message = status.iter().flatten().next().unwrap();
        assert_eq!(message, "the mask/zone unit stopped the acquisition with these settings: tested in software");
        let wake = outcome.events.iter().position(|e| *e == Event::SourceWake).unwrap();
        let arm = outcome.events[wake..].iter().position(is_arm).unwrap();
        assert_eq!(trigger_plans(&outcome.events[wake..wake + arm]), 1);
    }
}

fn edit_mask_unit_while_held(initially_blocked: bool) -> (Outcome, [Option<String>; 3]) {
    use super::super::super::comparator::{mask_map, unit_key};
    use std::{cell::RefCell, rc::Rc};

    let _scope = Scope::new();
    let want = settings();
    let edit = Settings { tdiv: 1e-6, ..want.clone() };
    let out = requested(&want, Identity::default(), false);
    let (mut source, (mut io, trace)) = sim_source(&want, &out, "");
    let started = io.clock.now();
    let key = unit_key(if initially_blocked { &want } else { &edit }, mask_map());
    for _ in 0..2 {
        source.batch.guard.batch(true, true, key, started);
    }
    source.batch.guard.batch(false, false, key, started);
    let statuses = Rc::new(RefCell::new([None, None, None]));
    let snapshots = statuses.clone();
    let shared = out.clone();
    let mut stopped = false;
    io.clock.drive(move |clock, pause| match pause {
        Pause::Park if !stopped => {
            stopped = true;
            clock.stop();
            let mut g = lock(&shared);
            snapshots.borrow_mut()[0] = g.unit_blocked.clone();
            g.want = Some(edit.clone());
        }
        Pause::Wait(HELD_POLL) => {
            snapshots.borrow_mut()[1] = lock(&shared).unit_blocked.clone();
            clock.release();
        }
        Pause::Park => {
            let mut g = lock(&shared);
            snapshots.borrow_mut()[2] = g.unit_blocked.clone();
            g.record_flow.shutdown = true;
        }
        Pause::Wait(_) => {}
    });
    source.run(&mut io);
    let outcome = Outcome::snapshot(&mut trace.borrow_mut(), &lock(&out), io.clock.now().duration_since(started));
    (outcome, statuses.take())
}

#[test]
fn core_trim_due_after_held_restore_waits_for_run() {
    let outcome = twice(trim_after_restore);
    let wake = outcome.events.iter().position(|e| *e == Event::SourceWake).unwrap();
    let trim = outcome.events.iter().position(|e| *e == Event::Trim(1)).unwrap();
    let arm = outcome.events[wake..].iter().position(is_arm).unwrap();
    assert!(wake < trim && trim < wake + arm);
    assert_eq!(outcome.events.iter().filter(|e| matches!(e, Event::Trim(_))).count(), 1);
    assert_eq!(trigger_plans(&outcome.events[wake..]), 0);
}

fn trim_after_restore() -> Outcome {
    let _scope = Scope::new();
    let want = settings();
    let out = requested(&want, Identity::default(), false);
    let (mut source, (mut io, trace)) = sim_source(&want, &out, "cal.core_offset = tight");
    io.cal_adc = adc::CalAdc::from_payload(vec![0; 0x738]);
    let started = io.clock.now();
    let shared = out.clone();
    let mut stopped = false;
    let mut passes = 0;
    io.clock.drive(move |clock, pause| match pause {
        Pause::Park if !stopped => {
            stopped = true;
            clock.stop();
        }
        Pause::Wait(HELD_POLL) => {
            passes += 1;
            match passes {
                1 => lock(&shared).cal_req = Some((vec![0], true, false)),
                2 => clock.advance(super::super::super::trim::TRIM_HOLD),
                _ => clock.release(),
            }
        }
        Pause::Park => lock(&shared).record_flow.shutdown = true,
        Pause::Wait(_) => {}
    });
    source.run(&mut io);
    let mut trace = trace.borrow_mut();
    let g = lock(&out);
    Outcome::snapshot(&mut trace, &g, io.clock.now().duration_since(started))
}

#[test]
fn trim_due_during_single_waits_for_run_and_its_new_configuration_to_settle() {
    let outcome = twice(trim_after_single);
    let trace = &outcome.events;
    let wake = trace.iter().position(|event| *event == Event::SourceWake).unwrap();
    let first_run_arm = wake + trace[wake..].iter().position(is_arm).unwrap();
    let trim = trace.iter().position(|event| *event == Event::Trim(1)).unwrap();
    let next_run_arm = trim + trace[trim..].iter().position(is_arm).unwrap();
    assert!(wake < first_run_arm && first_run_arm < trim && trim < next_run_arm);
    assert_eq!(trace.iter().filter(|event| matches!(event, Event::Trim(_))).count(), 1);
}

fn trim_after_single() -> Outcome {
    use super::super::super::trim::TRIM_HOLD;

    const SINGLE_POLL: Duration = Duration::from_millis(2);
    let _scope = Scope::new();
    let want = Settings { auto: false, single: true, tdiv: 0.2, depth: 14_000, ..settings() };
    let out = requested(&want, Identity::default(), false);
    let (mut source, (mut io, trace)) = sim_source(&want, &out, "cal.core_offset = tight");
    io.clock.stop_triggers();
    io.cal_adc = adc::CalAdc::from_payload(vec![0; 0x738]);
    let started = io.clock.now();
    let shared = out.clone();
    let mut single_triggered = false;
    let mut single_polls = 0;
    let mut run_triggered = false;
    let mut run_published = false;
    io.clock.drive(move |clock, pause| match pause {
        Pause::Wait(SINGLE_POLL) if single_polls == 0 => single_polls += 1,
        Pause::Wait(SINGLE_POLL) => {
            lock(&shared).queue.clear();
            clock.stop();
        }
        Pause::Wait(HELD_POLL) => {
            lock(&shared).want.as_mut().unwrap().single = false;
            clock.release();
        }
        Pause::Wait(_) if clock.capture_armed() && !single_triggered => {
            single_triggered = true;
            clock.advance(TRIM_HOLD);
            clock.record_triggers(1);
        }
        Pause::Wait(_) if clock.capture_armed() && run_published => {
            lock(&shared).record_flow.shutdown = true;
        }
        Pause::Wait(_) if clock.capture_armed() && single_polls > 0 && !run_triggered => {
            run_triggered = true;
            clock.advance(TRIM_HOLD);
            clock.record_triggers(u32::MAX);
        }
        Pause::Park => {
            let mut g = lock(&shared);
            assert!(!g.queue.is_empty());
            g.queue.clear();
            run_published = true;
        }
        Pause::Wait(_) => {}
    });
    source.run(&mut io);
    snapshot(&out, &io, &trace, started)
}

fn sim_source(want: &Settings, out: &HwShared, conf: &str) -> (AcqLoop<FixedPacing>, sim::Fixture) {
    let fixture = sim::fixture(want, out.clone(), (Run::Selection, 4, false), None);
    let switches = Switches::parse(&|_| None, conf, "acq.frame0 = keep", Default::default());
    let options = Spawn { mask: want.mask, program_afe: false, cal_dir: String::new(), start_up: None };
    let source =
        AcqLoop::new(options, switches, out.clone(), Service::default(), None, fixture.0.clock.now(), FixedPacing);
    (source, fixture)
}

#[test]
fn held_side_unit_disarms_before_the_view_can_reread_a_dropped_slot() {
    let (outcome, retained, replied) = twice(disarm_during_held_view);
    let stop = outcome.events.iter().position(|e| *e == Event::Stop).unwrap();
    let held = &outcome.events[stop + 1..];
    let disarm = held.iter().position(|e| matches!(e, Event::Write(0x4004, v) if v >> 31 != 0)).unwrap();
    let start = dvm::start(0, dvm::EXP)[0];
    let unit = held.iter().position(|e| *e == Event::Write(start.0, start.1)).unwrap();
    assert!(disarm < unit);
    assert!(!held.iter().any(|e| matches!(e, Event::Dma(_))));
    assert_eq!(held.last(), Some(&Event::Wait(HELD_POLL)));
    assert!(!retained && !replied);
    assert_eq!(outcome.latest, Some(Frame::stamp(4, 3)));
}

fn disarm_during_held_view() -> (Outcome, bool, bool) {
    let _scope = Scope::new();
    let want = Settings { tdiv: 10e-6, depth: 14_000, ..settings() };
    let out = requested(&want, Identity::default(), false);
    let (mut source, (mut io, trace)) = sim_source(&want, &out, "acq.prearm = on");
    let started = io.clock.now();
    capture(&mut source, &mut io);
    assert!(source.batch.armed.is_some() && source.replay.last_batch.is_some());
    io.clock.trigger_poll();
    io.clock.stop();
    // Enter directly at the held pass with an armed successor to exercise side-unit invalidation.
    source.replay.paused = true;
    {
        let mut g = lock(&out);
        g.stop_view = Some(StopView { capture: 0, scale: 1e-6, delay: 0.0 });
        g.dvm_want = Some(DvmWant { ch: 0, lane: None });
    }
    let shared = out.clone();
    io.clock.drive(move |_, pause| {
        if matches!(pause, Pause::Wait(HELD_POLL | VIEW_POLL)) {
            lock(&shared).record_flow.shutdown = true;
        }
    });
    source.run(&mut io);
    let g = lock(&out);
    let outcome = Outcome::snapshot(&mut trace.borrow_mut(), &g, io.clock.now().duration_since(started));
    (outcome, source.replay.last_batch.is_some(), g.stop_replies.frame.is_some())
}
