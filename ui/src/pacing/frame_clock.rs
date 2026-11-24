// SPDX-License-Identifier: GPL-2.0-or-later

use super::{
    presentation::{Picture, FLIP_TIMEOUT},
    target::{CostView, Target},
    TIMER_PERIOD,
};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    OnceLock,
};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum ClockMode {
    Timer,
    #[default]
    Flip,
}

impl ClockMode {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Flip => "flip",
            Self::Timer => "timer",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum FlipDivisor {
    Two = 2,
    #[default]
    Three = 3,
    Four = 4,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Flip {
    pub(crate) sequence: u32,
    pub(crate) at: Instant,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FrameSlot {
    Ready,
    HelperBusy,
    PanelBusy,
}

#[derive(Default)]
struct MissedSteps {
    total: u64,
    at_step: u64,
    helper: u64,
}

struct Step {
    at: Instant,
    vblank: Option<Flip>,
}

struct Cadence {
    refresh: Duration,
    divisor: u32,
    period: Duration,
    target_refreshes: u32,
    adaptive: Option<Target>,
}

pub(crate) struct FrameClock {
    mode: ClockMode,
    cadence: Cadence,
    phase: u32,
    last_step: Step,
    resume_after: Option<Instant>,
    idle_since: Instant,
    last_flip: Option<Flip>,
    flip_timeout: Instant,
    fallback: bool,
    deferred: Option<(FrameSlot, Flip)>,
    skipped: MissedSteps,
}

impl FrameClock {
    pub(crate) fn new(mode: ClockMode, divisor: FlipDivisor, now: Instant, refresh: Duration) -> Self {
        let refresh = if refresh.is_zero() { TIMER_PERIOD / 2 } else { refresh };
        let divisor = divisor as u32;
        let period = if mode == ClockMode::Timer { TIMER_PERIOD } else { refresh * divisor };
        let budget = crate::drawpipe::budget::pipeline_budget(period);
        let target_refreshes = (budget.as_nanos() / refresh.as_nanos()).clamp(1, u128::from(u32::MAX)) as u32;
        let adaptive = (mode == ClockMode::Flip && divisor == 3).then(Target::default);
        Self {
            mode,
            cadence: Cadence { refresh, divisor, period, target_refreshes, adaptive },
            phase: 0,
            last_step: Step { at: now, vblank: None },
            resume_after: None,
            idle_since: now,
            last_flip: None,
            flip_timeout: now + FLIP_TIMEOUT,
            fallback: false,
            deferred: None,
            skipped: MissedSteps::default(),
        }
    }

    pub(crate) fn step_finished(&mut self, now: Instant) {
        self.idle_since = now;
    }

    pub(crate) fn period(&self) -> Duration {
        self.cadence.period
    }

    /// Returns elapsed time for a due step and whether the clock just entered timer fallback.
    #[cfg(test)]
    pub(crate) fn advance(&mut self, now: Instant, flip: Option<Flip>) -> (Option<Duration>, bool) {
        self.advance_available(now, flip, FrameSlot::Ready)
    }

    pub(crate) fn advance_available(
        &mut self,
        now: Instant,
        flip: Option<Flip>,
        slot: FrameSlot,
    ) -> (Option<Duration>, bool) {
        let was_fallback = self.fallback;
        let ready = match self.mode {
            ClockMode::Timer => slot == FrameSlot::Ready && now >= self.timer_deadline(),
            ClockMode::Flip => self.flip_ready(now, flip, slot),
        };
        let elapsed = ready.then(|| {
            let period = self.period();
            let elapsed = now.duration_since(self.last_step.at);
            self.last_step = Step { at: now, vblank: self.deferred.take().map(|(_, edge)| edge) };
            if self.mode == ClockMode::Timer || self.fallback {
                let missed = (elapsed.as_nanos() / period.as_nanos()).saturating_sub(1);
                let already_counted = self.skipped.total - self.skipped.at_step;
                self.skipped.total += (missed as u64).saturating_sub(already_counted);
            }
            self.skipped.at_step = self.skipped.total;
            self.resume_after = self.fallback.then(|| now + period);
            elapsed
        });
        (elapsed, self.fallback && !was_fallback)
    }

    fn flip_ready(&mut self, now: Instant, flip: Option<Flip>, slot: FrameSlot) -> bool {
        if let Some(flip) = flip.filter(|event| self.last_flip.is_none_or(|last| event.sequence != last.sequence)) {
            let divisor = u64::from(self.cadence.divisor);
            let refreshes = self.last_flip.map_or(u64::from(flip.sequence) % divisor, |last| {
                u64::from(self.phase) + u64::from(flip.sequence.wrapping_sub(last.sequence))
            });
            self.phase = (refreshes % divisor) as u32;
            let can_defer = slot != FrameSlot::Ready
                && flip.at >= self.last_step.at
                && self.idle_since.saturating_duration_since(self.last_step.at) < self.period()
                && self.resume_after.is_none();
            let due = self.phase == 0
                && (flip.at >= self.flip_ready_after() || can_defer)
                && now.saturating_duration_since(flip.at) < self.period();
            let periods = refreshes / divisor;
            // Timer fallback supplies its own steps until the flip clock resumes.
            if self.resume_after.is_none() {
                let superseded = self.deferred.filter(|_| periods > 0);
                self.skipped.total += periods.saturating_sub(u64::from(due)) + u64::from(superseded.is_some());
                self.skipped.helper += u64::from(superseded.is_some_and(|(slot, _)| slot == FrameSlot::HelperBusy));
            }
            // Keep one due step until the display can accept it, even between vblanks.
            if periods > 0 || due {
                self.deferred = due.then_some((slot, flip));
            }
            self.last_flip = Some(flip);
            self.flip_timeout = now + FLIP_TIMEOUT;
            self.fallback = false;
        } else {
            self.fallback = now >= self.flip_timeout;
        }
        if self.fallback {
            self.deferred = None;
            return slot == FrameSlot::Ready && now >= self.timer_deadline();
        }
        slot == FrameSlot::Ready && self.deferred.is_some()
    }

    pub(crate) fn seal(&self) -> Picture {
        Picture::new(self.last_step.at, self.last_step.vblank, self.cadence.refresh, self.target_refreshes())
    }

    pub(crate) fn select_view(&mut self, view: CostView) {
        if let Some(target) = &mut self.cadence.adaptive {
            target.select_view(view);
        }
    }

    pub(crate) fn observe_ready(&mut self, at: Instant, cost: Duration) {
        if let Some(target) = &mut self.cadence.adaptive {
            target.observe(at, cost, self.cadence.refresh);
        }
    }

    fn target_refreshes(&self) -> u32 {
        self.cadence.adaptive.as_ref().map_or(self.cadence.target_refreshes, Target::refreshes)
    }

    pub(crate) fn skipped(&self) -> u64 {
        self.skipped.total
    }

    pub(super) fn helper_skipped(&self) -> u64 {
        self.skipped.helper
    }

    pub(crate) fn wait_until(&self) -> Instant {
        match self.mode {
            ClockMode::Timer => self.timer_deadline(),
            ClockMode::Flip if self.fallback => self.timer_deadline(),
            ClockMode::Flip => self.flip_timeout,
        }
    }

    pub(crate) fn deadline(&self, now: Instant) -> Instant {
        if self.deferred.is_some() {
            return now;
        }
        if self.mode == ClockMode::Timer || self.fallback {
            return self.timer_deadline();
        }
        let next = self.last_flip.map_or(self.flip_timeout, |flip| {
            let refreshes = self.cadence.divisor - self.phase;
            at_or_after(flip.at + self.cadence.refresh * refreshes, self.period(), now.max(self.flip_ready_after()))
        });
        let fallback = self.flip_timeout.max(self.timer_deadline());
        if next >= self.flip_timeout {
            fallback
        } else {
            next
        }
    }

    fn flip_ready_after(&self) -> Instant {
        self.resume_after.map_or(self.idle_since, |resume| self.idle_since.max(resume))
    }

    fn timer_deadline(&self) -> Instant {
        self.last_step.at + self.period()
    }
}

fn at_or_after(first: Instant, period: Duration, at: Instant) -> Instant {
    if first >= at {
        return first;
    }
    let remainder = at.duration_since(first).as_nanos() % period.as_nanos();
    let fraction = Duration::from_nanos(u64::try_from(remainder).expect("the remainder is shorter than a frame"));
    if fraction.is_zero() {
        at
    } else {
        at + period - fraction
    }
}

#[derive(Default)]
pub(crate) struct PictureRate {
    window: Option<(Instant, u64)>,
}

impl PictureRate {
    pub(crate) fn observe(&mut self, at: Instant) -> Option<f32> {
        let (start, intervals) = self.window.get_or_insert((at, 0));
        let elapsed = at.saturating_duration_since(*start);
        if elapsed.is_zero() {
            return None;
        }
        *intervals += 1;
        if elapsed < Duration::from_secs(1) {
            return None;
        }
        let rate = *intervals as f32 / elapsed.as_secs_f32();
        self.window = Some((at, 0));
        Some(rate)
    }
}

pub(super) struct ClockPublication {
    origin: Instant,
    deadline_ns: AtomicU64,
    period: OnceLock<Duration>,
    fallbacks: AtomicU64,
    skipped: AtomicU64,
    helper_skipped: AtomicU64,
    target_refreshes: AtomicU64,
}

impl ClockPublication {
    pub(super) fn new(origin: Instant) -> Self {
        Self {
            origin,
            deadline_ns: AtomicU64::new(0),
            period: OnceLock::new(),
            fallbacks: AtomicU64::new(0),
            skipped: AtomicU64::new(0),
            helper_skipped: AtomicU64::new(0),
            target_refreshes: AtomicU64::new(0),
        }
    }

    pub(super) fn publish(&self, clock: &FrameClock, now: Instant) {
        // Flip pacing and its fallback share the mode-derived period for the lifetime of the display.
        self.period.get_or_init(|| clock.period());
        let ns = clock.deadline(now).saturating_duration_since(self.origin).as_nanos();
        let ns = ns.clamp(1, u128::from(u64::MAX)) as u64;
        self.skipped.store(clock.skipped(), Ordering::Relaxed);
        self.helper_skipped.store(clock.helper_skipped(), Ordering::Relaxed);
        self.target_refreshes.store(u64::from(clock.target_refreshes()), Ordering::Relaxed);
        self.deadline_ns.store(ns, Ordering::Release);
    }

    pub(super) fn period(&self) -> Duration {
        self.period.get().copied().unwrap_or(TIMER_PERIOD)
    }

    pub(super) fn next_step_after(&self, t: Instant) -> Option<Instant> {
        let ns = self.deadline_ns.load(Ordering::Acquire);
        (ns != 0).then(|| at_or_after(self.origin + Duration::from_nanos(ns), self.period(), t))
    }

    pub(super) fn note_fallback(&self) {
        self.fallbacks.fetch_add(1, Ordering::Relaxed);
    }

    pub(super) fn fallbacks(&self) -> u64 {
        self.fallbacks.load(Ordering::Relaxed)
    }

    pub(super) fn skipped(&self) -> u64 {
        self.skipped.load(Ordering::Relaxed)
    }

    pub(super) fn helper_skipped(&self) -> u64 {
        self.helper_skipped.load(Ordering::Relaxed)
    }

    pub(super) fn target_refreshes(&self) -> u64 {
        self.target_refreshes.load(Ordering::Relaxed)
    }
}

pub(crate) fn step_missed() -> u64 {
    #[cfg(test)]
    if let Some(skipped) = TEST_SKIPPED.with(std::cell::Cell::get) {
        return skipped;
    }
    super::CLOCK.skipped()
}

#[cfg(test)]
thread_local! {
    static TEST_SKIPPED: std::cell::Cell<Option<u64>> = const { std::cell::Cell::new(None) };
}

#[cfg(test)]
pub(crate) fn with_test_clock<T>(clock: &FrameClock, now: Instant, read: impl FnOnce() -> T) -> T {
    struct Restore(Option<u64>);
    impl Drop for Restore {
        fn drop(&mut self) {
            TEST_SKIPPED.set(self.0);
        }
    }
    let publication = ClockPublication::new(now);
    publication.publish(clock, now);
    let _restore = Restore(TEST_SKIPPED.replace(Some(publication.skipped())));
    read()
}

#[cfg(test)]
mod tests;
