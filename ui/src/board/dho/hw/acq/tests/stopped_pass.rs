// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

const HELD_POLL: Duration = Duration::from_millis(5);

#[test]
fn held_pass_without_a_view_writes_no_registers_for_any_request() {
    for levels in [&[][..], &[0.5][..]] {
        let outcome = twice(|| stopped_pass(levels, false));
        let stop = outcome.events.iter().position(|event| *event == Event::Stop).unwrap();
        assert_eq!(outcome.events[stop + 1..], [Event::Wait(HELD_POLL)]);
        assert_eq!(outcome.final_ack, 7);
        assert_eq!(outcome.latest, Some(Frame::stamp(4, 3)));
    }
}

#[test]
fn run_applies_only_the_latest_held_request_before_the_first_arm() {
    for levels in [&[0.5][..], &[0.25, 0.5][..]] {
        let outcome = twice(|| stopped_pass(levels, true));
        let stop = outcome.events.iter().position(|event| *event == Event::Stop).unwrap();
        let wake = outcome.events.iter().position(|event| *event == Event::SourceWake).unwrap();
        let held = &outcome.events[stop + 1..wake];
        assert_eq!(held.len(), levels.len());
        assert!(held.iter().all(|event| *event == Event::Wait(HELD_POLL)), "{held:?}");
        let run = &outcome.events[wake + 1..];
        let arms: Vec<_> = run
            .iter()
            .enumerate()
            .filter(|(_, event)| matches!(event, Event::Write(0x400c, value) if value & 0x6200_0000 == 0x2200_0000))
            .map(|(index, _)| index)
            .collect();
        assert_eq!(arms.len(), 2);
        let expected = [Event::Write(0x2004, 0x85dc_8ea6), Event::Write(0x2008, 0x8ea6_9770)];
        let level_writes: Vec<_> =
            run.iter().filter(|event| matches!(event, Event::Write(0x2004 | 0x2008, _))).collect();
        assert_eq!(level_writes, expected.each_ref());
        assert!(run[..arms[0]].windows(expected.len()).any(|writes| writes == expected));
    }
}

fn stopped_pass(levels: &'static [f64], resume: bool) -> Outcome {
    let _scope = Scope::new();
    let want = settings();
    let out = requested(&want, Identity::default(), false);
    let fixture = sim::fixture(&want, out.clone(), (Run::Selection, 4, false), None);
    let shared = out.clone();
    let mut levels = levels.iter().copied();
    let mut stopped = false;
    let mut resumed_batches = 0;
    drive(&out, fixture, move |clock, pause| match pause {
        Pause::Park if !stopped => {
            clock.stop();
            stopped = true;
            let mut g = lock(&shared);
            assert!(g.stop_view.is_none());
            if let Some(level) = levels.next() {
                g.want.as_mut().unwrap().level = level;
            }
        }
        Pause::Wait(HELD_POLL) => {
            if let Some(level) = levels.next() {
                lock(&shared).want.as_mut().unwrap().level = level;
            } else if resume {
                clock.release();
            } else {
                lock(&shared).record_flow.shutdown = true;
            }
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
