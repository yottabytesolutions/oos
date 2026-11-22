// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::pacing::{ClockMode, FlipDivisor, FrameClock, FrameSlot};

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
        script.panel.stage(back, picture, script.at(done));
        script.poll(done, None);

        let vblank = u64::from(displayed) * 10;
        script.poll(vblank + 4, Some((displayed, vblank)));
    }
    assert!(script.requests.is_empty());
    assert_eq!((script.history.shown, script.history.late, clock.skipped()), (2, 0, 0));
    assert_eq!(
        script.history.stages[0].1,
        [Some(2000.0), Some(12000.0), Some(0.0), Some(4000.0), Some(20000.0), Some(4000.0)]
    );
    assert_eq!(script.history.trigger_flip, [30000.0, 40000.0]);
    let mut text = String::from(" stage.v1 trigger_composed=7/8/9");
    script.history.write(&mut text, 3);
    assert!(text.starts_with(" stage.v1 trigger_composed=7/8/9 present.v1 flip.shown=2 flip.late=0"));
    assert!(text.contains("flip.hz=25.0 helper.skipped=3"));
    assert!(text.contains("done_flip=6000/6000/6000"));
}

#[test]
fn a_busy_drm_slot_keeps_the_same_picture_until_retry_or_modeset() {
    let mut script = Script::new([(0, FlipRequest::Queued), (1, FlipRequest::Busy), (1, FlipRequest::Modeset)]);
    script.panel.stage(0, Picture::new(script.at(0)), script.at(1));
    script.poll(1, None);
    script.panel.stage(1, Picture::new(script.at(30)), script.at(33));
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
    script.panel.stage(1, Picture::new(script.at(0)), script.at(12));
    script.poll(12, None);
    script.poll(21, Some((2, 20)));
    assert_eq!(script.history.stages[0].1, [None, None, None, Some(8000.0), Some(20000.0), Some(8000.0)]);
    assert_eq!(script.history.shown, 1);
}

#[test]
fn two_completions_before_one_vblank_flip_in_order() {
    let mut script = Script::new([(1, FlipRequest::Queued), (2, FlipRequest::Queued)]);
    for (back, done) in [(1, 12), (2, 15)] {
        let mut picture = Picture::new(script.at(done - 1));
        picture.helper_done(script.at(done - 1), script.at(done));
        script.panel.stage(back, picture, script.at(done));
        script.poll(done, None);
    }
    assert!(!script.panel.ready_for_picture());
    script.poll(20, Some((2, 20)));
    script.poll(30, Some((3, 30)));
    assert!(script.requests.is_empty());
    assert_eq!((script.history.shown, script.history.late), (2, 1));
    assert_eq!(script.history.stages[1].1[5], Some(15_000.0));
}

#[test]
fn helper_completion_preserves_cadence_up_to_serial_capacity() {
    let refresh = Duration::from_micros(10_266);
    let period = refresh * 3;
    let start = Instant::now();
    let warmup_periods = 10;
    let end = start + period * (warmup_periods + 100);
    for helper_ms in [[5, 5], [12, 12], [25, 25], [35, 35], [70, 70], [35, 12]] {
        for compose_ms in [1, 3, 5] {
            let compose = Duration::from_millis(compose_ms);
            let helpers = helper_ms.map(Duration::from_millis);
            let mut durations = helpers.into_iter().cycle();
            let mut clock = FrameClock::new(ClockMode::Flip, FlipDivisor::Three, start, refresh);
            let mut sim = Simulation::default();
            let mut sequence = 1;
            let mut shown_at_warmup = 0;
            while let Some((now, edge)) = sim.tick(start, refresh, &mut sequence, end) {
                if edge.is_some_and(|flip| flip.sequence == warmup_periods * 3) {
                    shown_at_warmup = sim.history.shown;
                }
                let slot = if sim.rendering.is_some() {
                    FrameSlot::HelperBusy
                } else if sim.panel.ready_for_picture() {
                    FrameSlot::Ready
                } else {
                    FrameSlot::PanelBusy
                };
                if clock.advance_available(now, edge, slot).0.is_some() {
                    sim.submit(clock.seal(), now + compose, durations.next().unwrap());
                    clock.step_finished(now + compose);
                }
            }
            let shown = u128::from(sim.history.shown - shown_at_warmup);
            let label = format!("compose {compose_ms} ms, helper {helper_ms:?} ms");
            let pair = compose * 2 + helpers[0] + helpers[1];
            let capacity = ((period * 200).as_nanos() / pair.as_nanos()).min(100);
            if capacity == 100 {
                assert_eq!(shown, 100, "{label}");
                assert_eq!(clock.helper_skipped(), 0, "{label}");
            } else {
                assert!(shown.abs_diff(capacity) <= 1, "{label}: {shown}, capacity {capacity}");
                assert!(clock.helper_skipped() > 0, "{label}");
                assert_eq!(clock.helper_skipped(), clock.skipped(), "{label}");
            }
        }
    }
}

#[derive(Default)]
struct Simulation {
    panel: Panel,
    history: History,
    rendering: Option<(Instant, usize, Picture)>,
    requested: Option<usize>,
    scanout: usize,
    order: VecDeque<Instant>,
}

impl Simulation {
    fn tick(
        &mut self,
        start: Instant,
        refresh: Duration,
        sequence: &mut u32,
        end: Instant,
    ) -> Option<(Instant, Option<Flip>)> {
        let vblank = start + refresh * *sequence;
        let now = self.rendering.as_ref().map_or(vblank, |(done, _, _)| (*done).min(vblank));
        if now > end {
            return None;
        }
        let edge = (now == vblank).then(|| {
            let flip = Flip { sequence: *sequence, at: now };
            *sequence += 1;
            flip
        });
        let flip = edge.and_then(|edge| {
            self.scanout = self.requested.take()?;
            Some(edge)
        });
        if let Some((_, back, picture)) = self.rendering.take_if(|(done, _, _)| *done == now) {
            self.panel.stage(back, picture, now);
        }
        self.panel.service_with(
            now,
            flip,
            refresh,
            &mut |back| {
                assert_ne!(back, self.scanout);
                assert!(self.requested.replace(back).is_none());
                FlipRequest::Queued
            },
            &mut |outcome| {
                let Outcome::Shown(picture, _, flipped, _) = outcome else { panic!("unexpected flip outcome") };
                assert_eq!(self.order.pop_front(), Some(picture.seal));
                let elapsed = flipped - picture.ready;
                assert!(!elapsed.is_zero() && elapsed <= refresh, "completion to flip {elapsed:?}");
                self.history.record(outcome);
            },
        );
        Some((now, edge))
    }

    fn submit(&mut self, mut picture: Picture, submitted: Instant, helper: Duration) {
        assert!(self.panel.ready_for_picture());
        let back = (0..3).find(|&back| back != self.scanout && Some(back) != self.requested).unwrap();
        let done = submitted + helper;
        picture.composed = submitted;
        picture.helper_done(submitted, done);
        self.order.push_back(picture.seal);
        assert!(self.rendering.replace((done, back, picture)).is_none());
    }
}
