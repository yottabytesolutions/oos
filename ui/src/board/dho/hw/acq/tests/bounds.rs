// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use oos_hal::dho::scu::status::STOPPED;

const STATUS: u32 = 0x4034;
const DONE: u32 = 0x4080;
const WAITING: u32 = 0x12;
const HELD_POLL: Duration = Duration::from_millis(5);
const RETRY: Duration = Duration::from_millis(200);

#[test]
fn stop_acknowledges_after_one_disarm_poll_even_without_hardware_acknowledgement() {
    let outcome = twice(stop_without_ack);
    let trace = &outcome.events;
    let stop = trace.iter().position(|event| *event == Event::Stop).unwrap();
    let first = trace.iter().position(|event| *event == Event::Read(STATUS, WAITING)).unwrap();
    let end = first + trace[first..].iter().position(|event| matches!(event, Event::Write(_, _))).unwrap();
    assert!(stop < first);
    let poll = &trace[first..end];
    assert_eq!(poll.len(), 41);
    assert!(poll.chunks(2).all(|pair| matches!(
        pair,
        [Event::Read(STATUS, WAITING), Event::Wait(delay)] if *delay == Duration::from_micros(500)
    ) || pair == [Event::Read(STATUS, WAITING)]));
    assert_eq!(waiting_time(poll), Duration::from_millis(10));
    let stop_pulses =
        trace[end..].iter().filter(|event| matches!(event, Event::Write(0x4004, value) if value & (1 << 31) != 0));
    assert_eq!(stop_pulses.count(), 1);
    assert!(!trace[stop..].iter().any(|event| matches!(event, Event::Read(STATUS, flags) if flags & STOPPED != 0)));
    assert_eq!(trace.iter().filter(|event| **event == Event::Wait(HELD_POLL)).count(), 2);
    assert!(outcome.latest.is_none() && outcome.queue.is_empty());
    assert_eq!(outcome.final_ack, 7);
}

fn stop_without_ack() -> Outcome {
    let _scope = Scope::new();
    let want = Settings { auto: false, ..settings() };
    let out = requested(&want, Identity::default(), false);
    let fixture = sim::fixture(&want, out.clone(), (Run::Selection, 0, false), None);
    let shared = out.clone();
    let mut stopped = false;
    let mut held_polls = 0;
    drive(&out, fixture, move |clock, pause| match pause {
        Pause::Wait(delay) if delay == HELD_POLL => {
            let mut g = lock(&shared);
            assert_eq!(g.final_ack, 7);
            held_polls += 1;
            g.record_flow.shutdown = held_polls == 2;
        }
        Pause::Wait(_) if !stopped => {
            clock.stop();
            clock.suppress_stopped_status();
            stopped = true;
            assert_eq!(lock(&shared).final_ack, 0);
        }
        Pause::Wait(_) => {}
        Pause::Park => panic!("no record is available to park"),
    })
}

#[test]
fn serial_stop_cancels_a_missing_final_trigger_after_two_virtual_seconds() {
    let outcome = twice(serial_stop_without_trigger);
    let trace = &outcome.events;
    let stop = trace.iter().position(|event| *event == Event::Stop).unwrap();
    let disarm = trace.iter().position(|event| *event == Event::Read(STATUS, WAITING)).unwrap();
    assert_eq!(waiting_time(&trace[stop + 1..disarm]), Duration::from_secs(2));
    assert_eq!(outcome.final_ack, 7);
    assert!(outcome.latest.is_none() && outcome.queue.is_empty());
    assert!(!trace.iter().any(|event| matches!(event, Event::Dma(_))));
}

fn serial_stop_without_trigger() -> Outcome {
    let _scope = Scope::new();
    // Make the normal HAL trigger deadline longer than the STOP final-record allowance.
    let want = Settings { auto: false, tdiv: 0.2, depth: 14_000, ..settings() };
    let identity = Identity::default();
    let out = requested(&want, identity, false);
    {
        let mut g = lock(&out);
        g.record_flow.cost.set_identity(identity);
        g.record_flow.cost.observe(identity, 1, Duration::from_millis(50), Duration::ZERO);
    }
    let fixture = sim::fixture(&want, out.clone(), (Run::Selection, 0, false), None);
    let shared = out.clone();
    let mut stopped = false;
    drive(&out, fixture, move |clock, pause| match pause {
        Pause::Wait(HELD_POLL) => lock(&shared).record_flow.shutdown = true,
        Pause::Wait(_) if !stopped => {
            assert!(lock(&shared).record_flow.serial);
            clock.stop();
            stopped = true;
        }
        Pause::Wait(_) => {}
        Pause::Park => panic!("the final trigger never arrived"),
    })
}

#[test]
fn run_and_armed_single_retry_without_a_trigger_or_completion_acknowledgement() {
    for single in [false, true] {
        let outcome = twice(|| missing_trigger(single));
        let wake = outcome.events.iter().position(|event| *event == Event::SourceWake).unwrap();
        assert_eq!(trigger_plans(&outcome.events[..wake]), 1);
        assert_eq!(trigger_plans(&outcome.events[wake..]), 0);
        let retries: Vec<_> = outcome.events[wake + 1..].split(|event| *event == Event::Wait(RETRY)).collect();
        let [first, second, []] = retries.as_slice() else { panic!("expected two acquisition attempts and backoffs") };
        for attempt in [first, second] {
            let disarm = attempt.iter().position(|event| *event == Event::Read(STATUS, WAITING)).unwrap();
            let trigger_wait = waiting_time(&attempt[..disarm]);
            assert!(
                (Duration::from_secs(2)..Duration::from_micros(2_001_000)).contains(&trigger_wait),
                "{trigger_wait:?}"
            );
            assert_eq!(waiting_time(&attempt[disarm..]), Duration::from_millis(10));
            assert!(attempt[..disarm].contains(&Event::Read(DONE, 0)));
            assert!(!attempt.iter().any(|event| matches!(event, Event::Dma(_))));
        }
        assert_eq!(outcome.final_ack, 7);
        assert!(outcome.latest.is_none() && outcome.queue.is_empty());
    }
}

fn missing_trigger(single: bool) -> Outcome {
    let _scope = Scope::new();
    let want = Settings { auto: false, single, ..settings() };
    let out = requested(&want, Identity::default(), false);
    crate::wave::Source::Hw { shared: out.clone(), seen: 0 }.hw_hold(true, 7);
    let fixture = sim::fixture(&want, out.clone(), (Run::Selection, 0, false), None);
    let shared = out.clone();
    let mut retries = 0;
    drive(&out, fixture, move |clock, pause| match pause {
        Pause::Wait(HELD_POLL) => clock.release(),
        Pause::Wait(RETRY) => {
            let mut g = lock(&shared);
            assert!(!g.hold);
            assert_eq!(g.final_ack, 7);
            retries += 1;
            g.record_flow.shutdown = retries == 2;
        }
        Pause::Wait(_) => {}
        Pause::Park => panic!("the trigger never arrived"),
    })
}

pub(super) fn waiting_time(trace: &[Event]) -> Duration {
    trace.iter().filter_map(|event| if let Event::Wait(delay) = event { Some(*delay) } else { None }).sum()
}
