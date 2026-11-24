// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::pacing::{ClockMode, Flip, FlipDivisor, FrameClock, FrameSlot};

const REFRESH: Duration = Duration::from_millis(10);

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

    fn edge(&self, ms: u64) -> Flip {
        Flip { sequence: (ms / 10) as u32, at: self.at(ms) }
    }

    fn picture(&self, ms: u64, target_refreshes: u32) -> Picture {
        Picture::new(self.at(ms), Some(self.edge(ms)), REFRESH, target_refreshes)
    }

    fn poll(&mut self, ms: u64, flipped: Option<u64>) {
        let now = self.at(ms);
        self.panel.service_with(
            now,
            ms.is_multiple_of(10).then(|| self.edge(ms)),
            flipped.map(|at| self.edge(at)),
            REFRESH,
            &mut |back, _| {
                let (expected, outcome) =
                    self.requests.pop_front().expect("only the next scripted buffer can be flipped");
                assert_eq!(back, expected);
                outcome
            },
            &mut |outcome| self.history.record(outcome),
        );
    }

    fn advance_clock(&self, clock: &mut FrameClock, ms: u64, sequence: Option<u32>) -> (Option<Duration>, bool) {
        let now = self.at(ms);
        let edge = sequence.map(|sequence| Flip { sequence, at: now });
        let slot = if self.panel.ready_for_picture() { FrameSlot::Ready } else { FrameSlot::PanelBusy };
        clock.advance_available(now, edge, slot)
    }
}

#[test]
fn helper_completion_queues_its_own_picture_and_uses_the_event_timestamp() {
    let mut script = Script::new([(1, FlipRequest::Queued), (2, FlipRequest::Queued)]);
    let mut clock = FrameClock::new(ClockMode::Flip, FlipDivisor::Three, script.start, Duration::from_millis(10));
    for (seq, back, done, vblank) in [(3, 1, 46, 50), (6, 2, 74, 80)] {
        let at = script.at(u64::from(seq) * 10);
        assert!(clock.advance(at, Some(Flip { sequence: seq, at })).0.is_some());
        let mut picture = clock.seal();
        picture.composed = at + Duration::from_millis(2);
        picture.triggers[0] = Some(at - Duration::from_millis(10));

        // The scripted helper has not replied at the first wake.
        script.poll(u64::from(seq) * 10 + 10, None);
        script.poll(done - 1, None);
        assert_eq!(script.requests.len(), 3 - back);
        picture.helper_done(at + Duration::from_millis(4), script.at(done));
        script.panel.stage(back, picture, script.at(done));
        script.poll(done, None);

        script.poll(vblank + 4, Some(vblank));
    }
    assert!(script.requests.is_empty());
    assert_eq!((script.history.shown, script.history.late, clock.skipped()), (2, 0, 0));
    assert_eq!(
        script.history.stages[0].1,
        [Some(2000.0), Some(12000.0), Some(0.0), Some(4000.0), Some(20000.0), Some(4000.0)]
    );
    assert_eq!(script.history.trigger_flip, [30000.0, 30000.0]);
    let mut text = String::from(" stage.v1 trigger_composed=7/8/9");
    script.history.write(&mut text, 3, 2);
    assert!(text.starts_with(" stage.v1 trigger_composed=7/8/9 present.v2 flip.shown=2 flip.late=0"));
    assert!(text.contains("flip.hz=33.3 helper.skipped=3"));
    assert!(text.contains("flip.target_refreshes=2"));
    assert!(text.contains("done_flip=6000/6000/6000"));
}

mod cadence;

#[test]
fn a_busy_drm_slot_keeps_the_same_picture_until_retry_or_modeset() {
    let mut script = Script::new([(0, FlipRequest::Queued), (1, FlipRequest::Busy), (1, FlipRequest::Modeset)]);
    let mut clock = FrameClock::new(ClockMode::Flip, FlipDivisor::Three, script.start, Duration::from_millis(10));
    script.panel.stage(0, script.picture(0, 2), script.at(10));
    script.poll(10, None);
    script.panel.stage(1, script.picture(30, 2), script.at(33));
    script.poll(34, None);
    assert!(!script.panel.ready_for_picture());
    for (ms, sequence, retry, elapsed) in
        [(40, Some(4), Some(60), None), (60, Some(6), Some(70), None), (70, None, None, Some(70))]
    {
        script.poll(ms, None);
        assert_eq!(script.panel.wait_until(), retry.map(|at| script.at(at)));
        assert_eq!(script.advance_clock(&mut clock, ms, sequence), (elapsed.map(Duration::from_millis), false));
    }
    assert_eq!(script.history.timeout, 1);
    assert!(script.panel.ready_for_picture());
    assert_eq!(clock.skipped(), 0);
    assert_eq!((script.history.shown, script.history.modeset), (0, 1));
}

#[test]
fn cpu_picture_uses_the_same_flip_path_without_helper_timing() {
    let mut script = Script::new([(1, FlipRequest::Queued)]);
    script.poll(10, None);
    script.panel.stage(1, script.picture(0, 2), script.at(12));
    script.poll(12, None);
    script.poll(21, Some(20));
    assert_eq!(script.history.stages[0].1, [None, None, None, Some(8000.0), Some(20000.0), Some(8000.0)]);
    assert_eq!(script.history.shown, 1);
}

#[test]
fn late_counts_only_a_missed_target_refresh() {
    let start = Instant::now();
    let us = |micros| start + Duration::from_micros(micros);
    let refresh = Duration::from_micros(10_266);
    // A seal at sequence MAX targets sequence 1 across the DRM counter wrap.
    let seal = Flip { sequence: u32::MAX, at: start };
    for (done, time, sequence, late) in [(10_000, 20_530, 1, 0), (11_500, 20_530, 1, 0), (21_000, 30_800, 2, 1)] {
        let mut picture = Picture::new(start, Some(seal), refresh, 2);
        picture.helper_done(start, us(done));
        picture.ready = us(done);
        let mut history = History::default();
        history.record(Outcome::Shown(&picture, us(done), Flip { sequence, at: us(time) }, refresh));
        assert_eq!((history.shown, history.late), (1, late));
    }
}

#[test]
fn earlier_helper_completion_never_increases_late_flips() {
    let start = Instant::now();
    let seal = Flip { sequence: 6, at: start };
    for sequence in [8, 9, 10] {
        let flip = Flip { sequence, at: start + REFRESH * (sequence - seal.sequence) };
        let mut previous = 1;
        let flip_ms = (flip.at - start).as_millis() as u64;
        for done_ms in (0..flip_ms).rev() {
            let mut picture = Picture::new(start, Some(seal), REFRESH, 2);
            let done = start + Duration::from_millis(done_ms);
            picture.helper_done(start, done);
            picture.ready = done;
            let mut history = History::default();
            history.record(Outcome::Shown(&picture, done, flip, REFRESH));
            assert!(history.late <= previous);
            assert_eq!(history.late, u64::from(sequence > 8));
            previous = history.late;
        }
    }
}

#[test]
fn early_helper_waits_for_the_intermediate_vblank() {
    let mut script = Script::new([(1, FlipRequest::Queued), (2, FlipRequest::Queued), (0, FlipRequest::Queued)]);
    let mut clock = FrameClock::new(ClockMode::Flip, FlipDivisor::Three, script.start, REFRESH);
    for (seal, back, done) in [(30, 1, 32), (60, 2, 69), (90, 0, 103)] {
        let edge = script.edge(seal);
        assert!(clock.advance(edge.at, Some(edge)).0.is_some());
        script.poll(seal, None);
        let mut picture = clock.seal();
        picture.helper_done(edge.at, script.at(done));
        if done >= seal + 10 {
            script.poll(seal + 10, None);
        }
        script.panel.stage(back, picture, script.at(done));
        script.poll(done, None);
        if done < seal + 10 {
            assert_eq!(script.panel.wait_until(), Some(script.at(seal + 15)));
            script.poll(seal + 9, None);
            assert!(script.panel.flipping.is_none());
            script.poll(seal + 10, None);
        }
        assert_eq!(script.panel.flipping.as_ref().unwrap().0, script.at(done.max(seal + 10)));
        script.poll(seal + 20, Some(seal + 20));
    }
    assert_eq!((script.history.shown, script.history.late), (3, 0));
    for (a, b) in script.history.stages.iter().zip(script.history.stages.iter().skip(1)) {
        assert_eq!(b.0 - a.0, REFRESH * 3);
    }
}

#[test]
fn a_deferred_step_keeps_its_original_vblank_target() {
    let script = Script::new([]);
    let mut clock = FrameClock::new(ClockMode::Flip, FlipDivisor::Three, script.start, REFRESH);
    let due = script.edge(30);
    assert_eq!(clock.advance_available(due.at, Some(due), FrameSlot::HelperBusy).0, None);
    let started = script.at(42);
    assert!(clock.advance(started, Some(script.edge(40))).0.is_some());
    let picture = clock.seal();
    assert_eq!(picture.seal, started);
    assert_eq!(picture.target.sequence, Some(5));
    assert_eq!(picture.target.at, script.at(50));
}

#[test]
fn missing_vblank_events_have_a_bounded_timer_fallback() {
    let mut script = Script::new([(1, FlipRequest::Queued), (2, FlipRequest::Queued)]);
    script.panel.stage(1, script.picture(30, 2), script.at(33));
    script.poll(33, None);
    assert_eq!(script.panel.wait_until(), Some(script.at(45)));
    script.poll(44, None);
    assert!(!script.panel.ready_for_picture());
    script.poll(45, None);
    assert!(script.panel.ready_for_picture());
    script.poll(50, Some(50));
    assert_eq!((script.history.shown, script.history.late), (1, 0));
    // A timer-only step classifies completions by the window around its nominal target.
    let picture = Picture::new(script.at(75), None, REFRESH, 2);
    script.panel.stage(2, picture, script.at(80));
    script.poll(80, None);
    assert_eq!(script.panel.wait_until(), Some(script.at(90)));
    script.poll(90, None);
    script.poll(100, Some(100));
    assert_eq!((script.history.shown, script.history.late), (2, 0));
}

#[test]
fn fallback_request_after_the_target_refresh_is_late() {
    let mut script = Script::new([(1, FlipRequest::Queued)]);
    script.panel.stage(1, script.picture(30, 2), script.at(33));
    script.poll(33, None);
    script.poll(51, None);
    assert!(script.panel.ready_for_picture());
    script.poll(60, Some(60));
    assert_eq!((script.history.shown, script.history.late), (1, 1));
}

#[test]
fn two_completions_before_one_vblank_flip_in_order() {
    let mut script = Script::new([(1, FlipRequest::Queued), (2, FlipRequest::Queued)]);
    let mut clock = FrameClock::new(ClockMode::Flip, FlipDivisor::Two, script.start, Duration::from_millis(10));
    assert_eq!(script.advance_clock(&mut clock, 10, Some(1)), (None, false));
    script.poll(10, None);
    for (back, done) in [(1, 12), (2, 15)] {
        let mut picture = script.picture(10, 1);
        picture.helper_done(script.at(done - 1), script.at(done));
        script.panel.stage(back, picture, script.at(done));
        script.poll(done, None);
    }
    assert!(!script.panel.ready_for_picture());
    for (ms, panel, edge, elapsed) in
        [(20, None, Some(2), None), (21, Some(20), None, Some(21)), (30, Some(30), Some(3), None)]
    {
        script.poll(ms, panel);
        assert_eq!(script.advance_clock(&mut clock, ms, edge), (elapsed.map(Duration::from_millis), false));
    }
    assert!(script.requests.is_empty());
    assert_eq!(clock.skipped(), 0);
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
        let flipped = edge.filter(|_| {
            let Some(back) = self.requested.take() else { return false };
            self.scanout = back;
            true
        });
        if let Some((_, back, picture)) = self.rendering.take_if(|(done, _, _)| *done == now) {
            self.panel.stage(back, picture, now);
        }
        self.panel.service_with(
            now,
            edge,
            flipped,
            refresh,
            &mut |back, _| {
                assert_ne!(back, self.scanout);
                assert!(self.requested.replace(back).is_none());
                FlipRequest::Queued
            },
            &mut |outcome| {
                let Outcome::Shown(picture, requested, flip, _) = outcome else { panic!("unexpected flip outcome") };
                assert_eq!(self.order.pop_front(), Some(picture.seal));
                assert!(picture.target.reached(flip, 0), "flip before target at {flip:?}");
                let elapsed = flip.at - requested;
                assert!(!elapsed.is_zero() && elapsed <= refresh, "request to flip {elapsed:?}");
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
