// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use std::{cell::RefCell, rc::Rc};

const VIEW: StopView = StopView { capture: 0, scale: 1e-6, delay: 0.0 };
const VIEW_POLL: Duration = Duration::from_millis(20);

#[test]
fn first_wait_cancel_discards_every_in_range_slot() {
    for ahead in [false, true] {
        for previous in [1, 4] {
            twice(|| first_wait(ahead, previous, StopReport::Empty));
        }
    }
}

#[derive(Clone, Copy)]
enum StopReport {
    Empty,
    Burst,
    ClearsDone,
    Late,
    Unconfirmed,
    DoneUnavailable,
    ReadError,
}

#[test]
fn drops_in_range_slots_regardless_of_disarm_status() {
    for report in [
        StopReport::Burst,
        StopReport::ClearsDone,
        StopReport::Late,
        StopReport::Unconfirmed,
        StopReport::DoneUnavailable,
        StopReport::ReadError,
    ] {
        twice(|| first_wait(true, 4, report));
    }
}

fn first_wait(ahead: bool, previous: u32, report: StopReport) -> Vec<Event> {
    let (_scope, mut source, (mut io, trace)) = captured(ahead, previous);
    let start = trace.borrow().events.len();
    io.clock.stop_next_fill(0);
    match report {
        StopReport::Empty => {}
        StopReport::Burst => io.clock.drive(triggers_during_disarm(&trace, u32::MAX)),
        StopReport::ClearsDone => {
            io.clock.drive(triggers_during_disarm(&trace, 1));
            io.clock.stop_clears_done();
        }
        StopReport::Late => io.clock.delay_stopped_status(Duration::from_micros(150)),
        StopReport::Unconfirmed => io.clock.suppress_stopped_status(),
        StopReport::DoneUnavailable => io.clock.fail_done_after_stop(),
        StopReport::ReadError => io.clock.error_reading_done_after_stop(),
    }
    capture(&mut source, &mut io);
    check_disarm(&trace.borrow().events[start..], report);
    assert!(!io.clock.capture_armed());
    if matches!(report, StopReport::ReadError) {
        assert_eq!(
            lock(&source.out).status.latest(),
            "stop not confirmed by the hardware: read 0x4080: simulated play-info failure"
        );
    }
    check_disarm_counters(report);
    check_reread(&mut source, &mut io, &trace, false);
    trace.take().events
}

#[test]
fn refused_serial_reads_discard_slots_beyond_the_fill_count() {
    for mode in [Mode::Normal, Mode::Average { count: 4 }] {
        twice(|| refused_read(mode));
    }
}

#[test]
fn force_cancels_a_consumed_successor_before_a_later_stop() {
    twice(|| {
        let (_scope, mut source, (mut io, trace)) = captured(true, 4);
        let start = trace.borrow().events.len();
        io.clock.stop_triggers();
        let out = source.out.clone();
        let mut cancel = true;
        let mut burst = triggers_during_disarm(&trace, u32::MAX);
        io.clock.drive(move |clock, pause| {
            if std::mem::take(&mut cancel) {
                assert!(matches!(pause, Pause::Wait(_)));
                lock(&out).force = true;
            }
            burst(clock, pause);
        });
        capture(&mut source, &mut io);
        assert!(!lock(&source.out).hold);
        assert!(!io.clock.capture_armed());
        assert!(!source.batch.armed.is_some());
        {
            let events = &trace.borrow().events[start..];
            assert!(!events.iter().any(is_arm));
            check_disarm(events, StopReport::Burst);
        }
        io.clock.stop();
        check_reread(&mut source, &mut io, &trace, false);
        trace.take().events
    });
}

#[test]
fn disarming_a_smaller_unread_successor_preserves_the_last_slot() {
    twice(smaller_successor);
}

#[test]
fn successor_stop_polls_with_shared_unlocked() {
    twice(|| {
        let (_scope, mut source, (mut io, trace)) = captured(true, 4);
        assert!(source.batch.armed.is_some());
        io.clock.stop();
        io.clock.suppress_stopped_status();
        let out = source.out.clone();
        let polled = Rc::new(std::cell::Cell::new(false));
        let probe = polled.clone();
        io.clock.drive(move |_, _| {
            assert!(out.try_lock().is_ok(), "the successor must stop outside the Shared lock");
            probe.set(true);
        });
        let request = Request::read(&lock(&source.out));
        assert!(source.reconfigure(&mut io, &request, &mut false).is_none());
        assert!(polled.get());
        assert!(lock(&source.out).status.latest().starts_with("stop not confirmed by the hardware"));
        trace.take().events
    });
}

#[test]
fn an_in_range_slot_is_discarded_if_the_failed_capture_was_not_disarmed() {
    let (_scope, mut source, _) = captured(false, 4);
    source.replay.invalidate(&source.out, 4);
    assert_discarded(&source);
    let mut text = String::new();
    drawpipe::counters().acquisition.write_disarms(&mut text);
    assert!(text.contains(" total=0 "), "{text}");
    assert!(text.ends_with(" dropped=1"), "{text}");
}

fn smaller_successor() -> Vec<Event> {
    let (_scope, mut source, (mut io, trace)) = captured(false, 4);
    let (settings, params) = source.configured.current.clone().unwrap();
    let plan = ArmPlan::new(&settings, Identity::default(), false, None, false);
    plan.prearm(
        &mut source.batch.dma,
        &mut source.batch.armed,
        None,
        Some(Grant::Overlapping(2)),
        || false,
        |settings, _| {
            let steps = io.state.arm_only_steps(settings, &params);
            io.execute(&steps).is_ok()
        },
    );
    assert!(source.batch.armed.is_some());
    io.clock.record_triggers(2);
    io.clock.stop();
    check_reread(&mut source, &mut io, &trace, true);
    assert!(!source.batch.armed.is_some());
    assert!(!io.clock.capture_armed());
    let mut text = String::new();
    drawpipe::counters().acquisition.write_disarms(&mut text);
    assert!(text.contains(" total=1 "), "{text}");
    assert!(text.ends_with(" dropped=0"), "{text}");
    trace.take().events
}

#[test]
fn stop_view_before_hold_waits_without_arming_or_rereading() {
    let (_scope, mut source, (mut io, trace)) = captured(false, 4);
    let out = source.out.clone();
    let mut ui = crate::wave::Source::Hw { shared: out.clone(), seen: 0 };
    assert!(ui.hw_stop(Some(VIEW)).1.is_none());
    let start = trace.borrow().events.len();
    for _ in 0..2 {
        let request = Request::read(&lock(&out));
        let mut last_read = std::mem::take(&mut source.replay.fresh);
        assert!(source.reconfigure(&mut io, &request, &mut last_read).is_none());
        assert!(lock(&out).stop_replies.frame.is_none());
    }
    assert_eq!(trace.borrow().events[start..], [Event::Wait(VIEW_POLL), Event::Wait(VIEW_POLL)]);
    io.clock.stop();
    check_reread(&mut source, &mut io, &trace, true);
}

#[test]
fn a_view_arriving_during_first_wait_disarms_before_hold() {
    let (_scope, mut source, (mut io, trace)) = captured(true, 4);
    io.clock.stop_triggers();
    let out = source.out.clone();
    let mut ui = crate::wave::Source::Hw { shared: out.clone(), seen: 0 };
    io.clock.drive(move |_, pause| {
        if let Pause::Wait(_) = pause {
            assert!(ui.hw_stop(Some(VIEW)).1.is_none());
        }
    });
    let started = trace.borrow().events.len();
    capture(&mut source, &mut io);
    assert!(!lock(&out).hold);
    assert!(lock(&out).stop_replies.frame.is_none());
    assert!(trace.borrow().events[started..].iter().any(is_disarm));
    io.clock.stop();
    check_reread(&mut source, &mut io, &trace, false);
}

#[test]
fn single_wait_disarms_before_waiting_for_hold() {
    let (_scope, mut source, (mut io, trace)) = captured(true, 4);
    io.clock.trigger_poll();
    let start = trace.borrow().events.len();
    let request = {
        let mut g = lock(&source.out);
        g.want.as_mut().unwrap().single = true;
        Request::read(&g)
    };
    source.configured.current.as_mut().unwrap().0.single = true;
    source.batch.single_done =
        Some(SingleCompletion { pair: request.pair(), handoff: SingleHandoff::AwaitingUi(io.clock.now()) });
    assert!(source.reconfigure(&mut io, &request, &mut true).is_none());
    let events = &trace.borrow().events[start..];
    assert!(events.iter().any(is_disarm));
    assert_eq!(events.last(), Some(&Event::Wait(Duration::from_millis(2))));
    assert!(!events.iter().any(is_arm));
    assert_discarded(&source);
}

#[test]
fn average_clear_disarms_before_pulsing_the_average() {
    use oos_hal::dho::acq::AVG_CLEAR;

    let (_scope, mut source, (mut io, trace)) = captured(true, 4);
    io.clock.trigger_poll();
    let start = trace.borrow().events.len();
    let (settings, params) = source.configured.current.clone().unwrap();
    let average = Settings { mode: Mode::Average { count: 4 }, ..settings };
    source.batch.configured(average.mode, source.switches.board.avg_kind(), 0);
    let request = {
        let mut g = lock(&source.out);
        g.avg_clear = 1;
        // Interrupt the next arm after servicing the average clear.
        g.cal_req = Some((vec![0], true, false));
        Request::read(&g)
    };
    source.batch(&mut io, (&average, &params), request, true);
    let events = &trace.borrow().events[start..];
    let disarm = events.iter().position(is_disarm).unwrap();
    let clear = events.iter().position(|event| *event == Event::Write(AVG_CLEAR, 1)).unwrap();
    assert!(disarm < clear);
    assert!(!events.iter().any(is_arm));
    assert_discarded(&source);
}

#[test]
fn segments_disarm_before_starting_the_recording() {
    let (_scope, mut source, (mut io, trace)) = captured(true, 4);
    io.clock.trigger_poll();
    let start = trace.borrow().events.len();
    let (settings, params) = source.configured.current.clone().unwrap();
    let segmented = Settings { segments: Some(seq::Segments { n: 4, interval: 0.0 }), ..settings };
    lock(&source.out).seg.running = true;
    source.segments(&mut io, &segmented, &params, Identity::default());
    let events = &trace.borrow().events[start..];
    let disarm = events.iter().position(is_disarm).unwrap();
    let arm = events.iter().position(is_arm).unwrap();
    assert!(disarm < arm);
    assert_eq!(lock(&source.out).seg.phase, crate::board::SegPhase::Recording);
    assert_discarded(&source);
}

#[test]
fn rejected_handoff_disarms_the_unread_successor() {
    let scope = Scope::new();
    let want = Settings { auto: false, ..settings() };
    let id = Identity::default();
    let out = requested(&want, id, true);
    let intake = scope.gated_worker();
    let (mut io, trace) = sim::fixture(&want, out.clone(), (Run::Selection, 4, false), Some(intake));
    let mut source = source(&io, out.clone(), want.mask, true);
    let pipe = drawpipe::pipe().unwrap();
    pipe.publish_view(&picture::model(want.tdiv), picture::SIZE.0, picture::SIZE.1, source.switches.interp, id);
    capture(&mut source, &mut io);
    assert!(source.batch.armed.is_some());
    io.clock.drive(move |clock, pause| {
        if pause == Pause::Park {
            clock.trigger_poll();
            clock.stop();
            lock(&out).stop_view = Some(VIEW);
        }
    });
    capture(&mut source, &mut io);
    assert_discarded(&source);
    capture(&mut source, &mut io);
    {
        let events = &trace.borrow().events;
        let park = events.iter().position(|event| matches!(event, Event::Park(_))).unwrap();
        assert_eq!(events[..park].iter().filter(|event| is_arm(event)).count(), 3);
        assert!(events[park..].iter().any(is_disarm));
        assert!(!events[park..].iter().any(|event| matches!(event, Event::Dma(_))));
    }
    assert_eq!(RecordCounts::read().loss("lost.settings"), 4);
    io.clock.compose();
}

pub(super) fn is_disarm(event: &Event) -> bool {
    matches!(event, Event::Write(0x4004, value) if value >> 31 != 0)
}

fn assert_discarded(source: &AcqLoop<FixedPacing>) {
    assert!(!source.batch.armed.is_some());
    assert!(source.replay.last_batch.is_none());
    assert!(lock(&source.out).stop_replies.frame.is_none());
}

fn captured(ahead: bool, records: u32) -> (Scope, AcqLoop<FixedPacing>, sim::Fixture) {
    captured_mode(ahead, records, Mode::Normal)
}

fn captured_mode(ahead: bool, records: u32, mode: Mode) -> (Scope, AcqLoop<FixedPacing>, sim::Fixture) {
    let scope = Scope::new();
    let want = Settings { auto: false, tdiv: 10e-6, depth: 14_000, mode, ..settings() };
    let out = requested(&want, Identity::default(), false);
    let (mut io, trace) = sim::fixture(&want, out.clone(), (Run::Selection, records, false), None);
    let mut source = source(&io, out, want.mask, ahead);
    capture(&mut source, &mut io);
    assert_eq!(source.batch.armed.is_some(), ahead);
    assert_eq!(source.replay.last_batch.as_ref().unwrap().index, records - 1);
    lock(&source.out).queue.clear();
    (scope, source, (io, trace))
}

fn source(io: &Io<sim::Sim>, out: HwShared, mask: u32, ahead: bool) -> AcqLoop<FixedPacing> {
    let options = Spawn { mask, program_afe: false, cal_dir: String::new(), start_up: None };
    let prearm = if ahead { "acq.prearm = on" } else { "acq.prearm = off" };
    let switches = Switches::parse(&|_| None, prearm, "acq.frame0 = keep", Default::default());
    AcqLoop::new(options, switches, out, Service::default(), None, io.clock.now(), FixedPacing)
}

fn refused_read(mode: Mode) -> Vec<Event> {
    let ahead = mode == Mode::Normal;
    let (_scope, mut source, (mut io, trace)) = captured_mode(ahead, 4, mode);
    let out = source.out.clone();
    {
        let mut g = lock(&out);
        let identity = g.acquisition.identity();
        g.record_flow.cost.set_identity(identity);
        g.record_flow.cost.observe(identity, 1, Duration::from_millis(50), Duration::ZERO);
    }
    let start = trace.borrow().events.len();
    io.clock.stop_next_fill(2);
    let mut burst = triggers_during_disarm(&trace, u32::MAX);
    io.clock.drive(move |clock, pause| {
        if matches!(pause, Pause::Wait(_)) {
            lock(&out).stop_view = Some(VIEW);
        }
        burst(clock, pause);
    });
    capture(&mut source, &mut io);
    assert!(lock(&source.out).record_flow.serial);
    assert!(!source.batch.armed.is_some() && !io.clock.capture_armed());
    {
        let events = &trace.borrow().events;
        assert_eq!(events[start..].iter().any(is_arm), !ahead);
        assert!(events[..]
            .iter()
            .rev()
            .find_map(|event| match event {
                Event::Write(0x4018, last) => Some(*last >= 3),
                _ => None,
            })
            .unwrap());
        assert!(events[start..].contains(&Event::Read(0x4010, 1)));
        check_disarm(&events[start..], StopReport::Burst);
    }
    check_reread(&mut source, &mut io, &trace, false);
    trace.take().events
}

fn triggers_during_disarm(trace: &Rc<RefCell<sim::Trace>>, count: u32) -> impl FnMut(&sim::Virtual, Pause) + 'static {
    let trace = trace.clone();
    let mut triggered = false;
    move |clock, pause| {
        if !triggered
            && matches!(pause, Pause::Wait(_))
            && matches!(trace.borrow().events.iter().rev().nth(1), Some(Event::Read(0x4034, flags))
                if flags & oos_hal::dho::scu::status::WAIT_TRIG != 0)
        {
            clock.record_triggers(count);
            triggered = true;
        }
    }
}

fn check_disarm(events: &[Event], report: StopReport) {
    use oos_hal::dho::scu::{status::STOPPED, PLAYINFO_DONE};

    let pulse = events.iter().rposition(is_disarm).unwrap();
    assert!(matches!(events[pulse - 5], Event::Read(0x4034, _) | Event::Wait(_)));
    assert!(events[pulse - 4..pulse].iter().all(|event| matches!(event, Event::Write(0x4004, _))));
    let clear = events[pulse..].iter().position(|event| matches!(event, Event::Write(0x4080, _))).unwrap();
    let evidence = &events[pulse + 1..pulse + clear];
    let fsm = match report {
        StopReport::Unconfirmed => 0,
        StopReport::ClearsDone => 0x20c0,
        _ => STOPPED,
    };
    let done = match report {
        StopReport::Burst => Event::Read(0x4080, PLAYINFO_DONE),
        StopReport::DoneUnavailable => Event::Read(0x4080, u32::MAX),
        StopReport::ReadError => Event::ReadError(0x4080),
        _ => Event::Read(0x4080, 0),
    };
    assert!(matches!(evidence, [.., fsm_read, play_read]
        if *fsm_read == Event::Read(0x4034, fsm) && *play_read == done));
    let expected_us = match report {
        StopReport::Late => 150,
        StopReport::Unconfirmed => 500,
        _ => 0,
    };
    assert_eq!(super::bounds::waiting_time(evidence), Duration::from_micros(expected_us));
}

fn check_disarm_counters(report: StopReport) {
    let mut text = String::new();
    drawpipe::counters().acquisition.write_disarms(&mut text);
    let (unknown, no_completed_record, recorded) = match report {
        StopReport::Empty | StopReport::ClearsDone | StopReport::Late => (0, 1, 0),
        StopReport::Burst => (0, 0, 1),
        StopReport::Unconfirmed | StopReport::DoneUnavailable | StopReport::ReadError => (1, 0, 0),
    };
    assert_eq!(
        text,
        format!(" disarm.v1 total=1 unknown={unknown} no_completed_record={no_completed_record} recorded={recorded} dropped=1")
    );
}

fn check_reread(source: &mut AcqLoop<FixedPacing>, io: &mut Io<sim::Sim>, trace: &RefCell<sim::Trace>, survives: bool) {
    check_reread_stamp(source, io, trace, survives.then(|| Frame::stamp(5, 3)));
}

fn check_reread_stamp(
    source: &mut AcqLoop<FixedPacing>,
    io: &mut Io<sim::Sim>,
    trace: &RefCell<sim::Trace>,
    expected: Option<Frame>,
) {
    let before = trace.borrow().events.iter().filter(|event| matches!(event, Event::Dma(_))).count();
    assert_eq!(before, 1, "the canceled capture must not reach DMA");
    let published = {
        let mut g = lock(&source.out);
        assert!(g.hold);
        g.stop_view = Some(VIEW);
        g.frame.clone()
    };
    capture(source, io);
    let g = lock(&source.out);
    assert_eq!(g.final_ack, 7);
    assert!(std::sync::Arc::ptr_eq(&g.frame, &published));
    assert_eq!(g.stop_replies.frame.as_ref().map(|(_, frame)| Frame::read(frame)), expected);
    assert_eq!(
        trace.borrow().events.iter().filter(|event| matches!(event, Event::Dma(_))).count(),
        before + usize::from(expected.is_some())
    );
}
