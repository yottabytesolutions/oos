// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::pacing::{ClockMode, FlipDivisor, FrameClock};

struct Script {
    panel: Panel,
    history: History,
    requests: VecDeque<(usize, FlipRequest)>,
    start: Instant,
}

impl Script {
    fn new(requests: impl IntoIterator<Item = (usize, FlipRequest)>) -> Self {
        Self {
            panel: Panel::default(),
            history: History::default(),
            requests: requests.into_iter().collect(),
            start: Instant::now(),
        }
    }

    fn at(&self, ms: u64) -> Instant {
        self.start + Duration::from_millis(ms)
    }

    fn poll(&mut self, ms: u64, event: Option<(u32, u64)>) {
        let now = self.at(ms);
        let event = event.map(|(sequence, stamp)| Flip { sequence, at: self.at(stamp) });
        self.panel.service_with(
            now,
            event,
            Duration::from_millis(10),
            &mut |back| {
                let (expected, outcome) =
                    self.requests.pop_front().expect("only the next scripted buffer can be flipped");
                assert_eq!(back, expected);
                outcome
            },
            &mut |outcome| self.history.record(outcome),
        );
    }
}

#[test]
fn helper_completion_queues_its_own_picture_and_uses_the_event_timestamp() {
    let mut script = Script::new([(1, FlipRequest::Queued), (2, FlipRequest::Queued)]);
    let mut clock = FrameClock::new(ClockMode::Flip, FlipDivisor::Three, script.start, Duration::from_millis(10));
    for (seq, back, done, displayed) in [(3, 1, 46, 5), (6, 2, 84, 9)] {
        let at = script.at(u64::from(seq) * 10);
        assert!(clock.advance(at, Some(Flip { sequence: seq, at })).0.is_some());
        let mut picture = clock.seal();
        picture.composed = at + Duration::from_millis(2);
        picture.triggers[0] = Some(at - Duration::from_millis(10));

        // The scripted helper has not replied at the first wake.
        script.poll(done - 1, None);
        assert_eq!(script.requests.len(), 3 - back);
        picture.helper_done(at + Duration::from_millis(4), script.at(done));
        script.panel.stage(back, picture);
        script.poll(done, None);

        let vblank = u64::from(displayed) * 10;
        script.poll(vblank + 4, Some((displayed, vblank)));
    }
    assert!(script.requests.is_empty());
    assert_eq!((script.history.shown, script.history.late, clock.skipped()), (2, 1, 0));
    assert_eq!(script.history.stages[0], [Some(2000.0), Some(12000.0), Some(0.0), Some(4000.0), Some(20000.0)]);
    assert_eq!(script.history.trigger_flip, [30000.0, 40000.0]);
    let mut text = String::from(" stage.v1 trigger_composed=7/8/9");
    script.history.write(&mut text);
    assert!(text.starts_with(" stage.v1 trigger_composed=7/8/9 present.v1 flip.shown=2 flip.late=1"));
}

#[test]
fn a_busy_drm_slot_keeps_the_same_picture_until_retry_or_modeset() {
    let mut script = Script::new([(0, FlipRequest::Queued), (1, FlipRequest::Busy), (1, FlipRequest::Modeset)]);
    script.panel.stage(0, Picture::new(script.at(0), Some(2)));
    script.poll(1, None);
    script.panel.stage(1, Picture::new(script.at(30), Some(5)));
    script.poll(34, None);
    assert!(!script.panel.ready_for_picture());
    script.poll(51, None);
    assert_eq!(script.history.timeout, 1);
    assert_eq!(script.panel.wait_until(), Some(script.at(61)));
    script.poll(61, None);
    assert!(script.panel.ready_for_picture());
    assert_eq!((script.history.shown, script.history.modeset), (0, 1));
}

#[test]
fn cpu_picture_uses_the_same_flip_path_without_helper_timing() {
    let mut script = Script::new([(1, FlipRequest::Queued)]);
    script.panel.stage(1, Picture::new(script.at(0), Some(2)));
    script.poll(12, None);
    script.poll(21, Some((2, 20)));
    assert_eq!(script.history.stages[0], [None, None, None, Some(8000.0), Some(20000.0)]);
    assert_eq!(script.history.shown, 1);
}

#[test]
fn one_busy_seal_counts_a_missed_step_and_keeps_the_phase() {
    let script = Script::new([]);
    let mut clock = FrameClock::new(ClockMode::Flip, FlipDivisor::Three, script.start, Duration::from_millis(10));
    let mut steps = Vec::new();
    for sequence in 1..=9 {
        let at = script.at(u64::from(sequence) * 10);
        let ready = clock.advance_available(at, Some(Flip { sequence, at }), sequence != 6).0;
        if let Some(elapsed) = ready {
            steps.push(elapsed);
            clock.step_finished(at);
        }
    }
    assert_eq!(steps, [Duration::from_millis(30), Duration::from_millis(60)]);
    assert_eq!(clock.skipped(), 1);
}
