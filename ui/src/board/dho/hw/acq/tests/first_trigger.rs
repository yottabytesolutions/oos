// SPDX-License-Identifier: GPL-2.0-or-later

use super::{bounds::waiting_time, stopped_slots::is_disarm, *};
use oos_hal::dho::scu::PLAYINFO_DONE;

const US: fn(u64) -> Duration = Duration::from_micros;
const RECORDS: u32 = 3;

#[derive(Clone, Copy)]
enum Script {
    Complete { first: Duration, tail: Duration },
    Missing { auto: bool },
    Cancel { wait: usize, first_ready: bool },
}

#[test]
fn first_trigger_tracks_early_on_time_and_late_fill() {
    let complete = |first, tail| Script::Complete { first, tail };
    let early = twice(|| batch(complete(US(55), US(360))));
    let on_time = twice(|| batch(complete(US(80), US(400))));
    let late = twice(|| batch(complete(US(105), US(430))));
    for (outcome, first_reads) in [(&early, 2), (&on_time, 2), (&late, 3)] {
        assert_eq!(outcome.queue.len(), RECORDS as usize);
        let (first, ring) = waits(&outcome.events);
        assert_eq!(first.iter().filter(|e| matches!(e, Event::Read(0x4080, _))).count(), first_reads);
        assert_eq!(ring.iter().filter(|e| matches!(e, Event::Read(0x4010, _))).count(), 3);
        assert_eq!(first.last(), Some(&Event::Read(0x4080, PLAYINFO_DONE)));
        assert_eq!(ring.last(), Some(&Event::Read(0x4010, RECORDS - 1)));
        assert_eq!(writes_and_dma(outcome), writes_and_dma(&on_time));
    }
    assert_eq!(waiting_time(waits(&early.events).0), US(80));
    assert_eq!(waiting_time(waits(&on_time.events).0), US(80));
    assert_eq!(waiting_time(waits(&late.events).0), US(130));
}

#[test]
fn first_trigger_backs_off_until_the_original_timeout() {
    for auto in [false, true] {
        let outcome = twice(|| batch(Script::Missing { auto }));
        let poll = first_poll(&outcome.events);
        let diagnostic = poll.iter().position(|e| matches!(e, Event::Read(0x4030, _))).unwrap();
        let wait = &poll[..diagnostic];
        let delays: Vec<_> = wait.iter().filter_map(|e| if let Event::Wait(d) = e { Some(*d) } else { None }).collect();
        assert_eq!(delays[..5], [US(80), US(50), US(50), US(50), US(50)]);
        assert_eq!(delays[5], US(300));
        assert!(delays[6..].iter().all(|&delay| delay == US(500)));
        assert_eq!(waiting_time(wait), US(2_000_080));
        assert!(outcome.queue.is_empty());
        assert!(!outcome.events.iter().any(|e| matches!(e, Event::Dma(_))));
    }
}

#[test]
fn first_trigger_cancellation_disarms_in_each_wait_phase() {
    for (wait, first_ready, max) in [
        (1, false, Duration::ZERO),            // estimate
        (2, false, Duration::ZERO),            // fine poll
        (6, false, Duration::from_millis(10)), // coarse poll
        (2, true, Duration::from_millis(10)),  // ring fill
    ] {
        let outcome = twice(|| batch(Script::Cancel { wait, first_ready }));
        let events = first_poll(&outcome.events);
        let canceled = events.iter().position(|e| *e == Event::SettingsPublished).unwrap();
        let disarm = canceled + events[canceled..].iter().position(|e| matches!(e, Event::Read(0x4034, _))).unwrap();
        let pulse = disarm + events[disarm..].iter().position(is_disarm).unwrap();
        assert!(waiting_time(&events[canceled + 1..disarm]) <= max);
        assert!(matches!(events[disarm - 1], Event::Read(0x4080 | 0x4010, 0)));
        assert!(events[disarm..pulse]
            .iter()
            .all(|e| matches!(e, Event::Read(0x4034, _) | Event::Wait(_) | Event::Write(0x4004, _))));
        assert_eq!(events.iter().filter(|e| is_disarm(e)).count(), 1);
        assert!(!events.iter().any(|e| matches!(e, Event::Dma(_))));
        assert!(outcome.queue.is_empty());
    }
}

fn first_poll(events: &[Event]) -> &[Event] {
    let arm = events.iter().position(is_arm).unwrap();
    let poll = events[arm..].iter().position(|e| matches!(e, Event::Read(0x4080, _))).unwrap();
    &events[arm + poll..]
}

fn waits(events: &[Event]) -> (&[Event], &[Event]) {
    let poll = first_poll(events);
    let peek = poll.iter().position(|e| matches!(e, Event::Read(0x4034, _))).unwrap();
    let play = poll.iter().position(|e| matches!(e, Event::Write(_, _))).unwrap();
    (&poll[..peek], &poll[peek..play])
}

fn writes_and_dma(outcome: &Outcome) -> Vec<&Event> {
    outcome.events.iter().filter(|e| matches!(e, Event::Write(_, _) | Event::Dma(_))).collect()
}

fn batch(script: Script) -> Outcome {
    let _scope = Scope::new();
    let auto = matches!(script, Script::Missing { auto: true });
    let want = Settings { tdiv: 10e-6, depth: 100_000, auto, ..settings() };
    let out = requested(&want, Identity::default(), false);
    let (mut io, trace) = sim::fixture(&want, out.clone(), (Run::Selection, 0, false), None);
    let (mut source, started) = source(&out, &io);
    let request = Request::read(&lock(&out));
    let (mut b, p) = source.reconfigure(&mut io, &request, &mut false).unwrap();
    b.frames = RECORDS;
    assert_eq!(Duration::from_nanos(u64::from(p.memsel) * 1_000_000_000 / p.rate), US(160));
    let mut arm = None;
    let (mut waits, mut recorded) = (0, 0);
    let shared = out.clone();
    io.clock.drive(move |clock, pause| {
        let Pause::Wait(delay) = pause else { panic!("unexpected admission wait") };
        if !clock.capture_armed() {
            return;
        }
        let armed = *arm.get_or_insert_with(|| clock.now() - delay);
        waits += 1;
        let elapsed = clock.now() - armed;
        match script {
            Script::Complete { first, tail } => {
                let due = if elapsed >= tail { RECORDS } else { u32::from(elapsed >= first) };
                clock.record_triggers(due - recorded);
                recorded = due;
            }
            Script::Cancel { wait, first_ready } => {
                if first_ready && waits == 1 {
                    clock.record_triggers(1);
                }
                if waits == wait {
                    lock(&shared).want.as_mut().unwrap().level = 0.5;
                    clock.mark(Event::SettingsPublished);
                }
            }
            Script::Missing { .. } => {}
        }
    });
    let serial = matches!(script, Script::Cancel { first_ready: true, .. });
    let plan = ArmPlan::new(&b, request.identity, request.route, None, serial);
    let grant = if serial { Grant::Serial } else { Grant::Overlapping(RECORDS) };
    let timing = batch_timing::SourceTiming::new(None, grant);
    let capture = Capture { settings: &b, params: &p, request: &request, uncapped: RECORDS };
    source.capture(&mut io, capture, (&plan, None), (false, None, timing, false));
    assert!(!io.clock.capture_armed());
    snapshot(&out, &io, &trace, started)
}
