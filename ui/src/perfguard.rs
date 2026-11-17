// SPDX-License-Identifier: GPL-2.0-or-later

//! Best-of-rounds ratios remain the throughput guard. A separate p99 ratio checks tails.
//! Tail limits allow 50 % contention headroom over each throughput limit; they never
//! compare a candidate tail with the reference minimum. All rounds interleave the paths.

pub(crate) mod deep;
mod draw;
mod intake;
#[cfg(test)]
pub(crate) mod latency;
pub(crate) mod ring;

#[cfg(test)]
use crate::system::perfguard as samples;

#[cfg(test)]
pub use samples::ROUNDS;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[cfg(test)]
pub fn guard_pair(
    best_limit: f64,
    rounds: usize,
    runs: usize,
    reference: impl FnMut(),
    candidate: impl FnMut(),
) -> (Duration, Duration) {
    let pair = samples::measure(rounds, runs, reference, candidate);
    pair.check_tail(best_limit * 1.5);
    if best_limit.is_finite() {
        let thread = std::thread::current();
        let name = thread.name().expect("the test harness names each guard thread");
        pair.report(name.rsplit("::").next().expect("a guard thread has a name"));
    }
    pair.best
}

/// The time source of the intake, change, consumer, picture and shadow rings, supplied at construction (ADR 0019).
#[derive(Clone)]
pub(crate) struct Clock(Arc<dyn Fn() -> Instant + Send + Sync>);
impl Default for Clock {
    fn default() -> Self {
        Self(Arc::new(Instant::now))
    }
}
impl Clock {
    #[cfg(test)]
    pub(crate) fn new(now: impl Fn() -> Instant + Send + Sync + 'static) -> Self {
        Self(Arc::new(now))
    }
    pub(crate) fn now(&self) -> Instant {
        (self.0)()
    }
}

/// Shared tail samples for intake/change budgets. A fixed ring avoids timing allocations in the UI.
#[derive(Clone)]
pub struct StageTiming {
    clock: Clock,
    draw: draw::Timing,
    intake: [u64; 256],
    change: [u64; 256],
    picture: [u64; 256],
    count: usize,
    picture_count: usize,
    shadow: [u64; 256],
    split: intake::Timing,
    queued: QueuedTiming,
    current_shadow: Option<Duration>,
    consumers: ConsumerTiming,
}
#[derive(Clone, Default)]
struct QueuedTiming {
    shadow: u64,
    change: u64,
}
#[derive(Clone)]
struct ConsumerTiming {
    samples: [u64; 256],
    count: usize,
}
impl Default for ConsumerTiming {
    fn default() -> Self {
        Self { samples: [0; 256], count: 0 }
    }
}
impl Default for StageTiming {
    fn default() -> Self {
        Self::with_clock(Clock::default())
    }
}
impl StageTiming {
    pub(crate) fn with_clock(clock: Clock) -> Self {
        Self {
            clock,
            draw: draw::Timing::default(),
            intake: [0; 256],
            change: [0; 256],
            picture: [0; 256],
            shadow: [0; 256],
            count: 0,
            picture_count: 0,
            split: intake::Timing::default(),
            queued: QueuedTiming::default(),
            current_shadow: None,
            consumers: ConsumerTiming::default(),
        }
    }
}
impl StageTiming {
    /// A sample of the rings' clock: the start or end of a timed region.
    pub(crate) fn now(&self) -> Instant {
        self.clock.now()
    }
    /// Time from `start` to now on the rings' clock.
    pub(crate) fn since(&self, start: Instant) -> Duration {
        self.now().saturating_duration_since(start)
    }
    /// Disjoint per-step decay, fast, merge, records and grade durations, including skipped zeros.
    pub fn draw_step(&mut self, parts: [Duration; 5]) {
        self.draw.step(parts);
    }
    /// Rendering is separate from step; panel and HDMI view calls have independent rings.
    pub fn draw_render(&mut self, elapsed: Duration, view: bool) {
        self.draw.render(elapsed, view);
    }
    pub fn begin_change(&mut self) {
        self.current_shadow = Some(Duration::ZERO);
    }
    pub fn record_shadow(&mut self, duration: Duration) {
        if let Some(current) = &mut self.current_shadow {
            *current += duration;
        } else {
            self.queue_shadow(duration);
        }
    }
    pub fn end_change(&mut self) -> Duration {
        self.current_shadow.take().unwrap_or_default()
    }
    pub fn add(&mut self, intake: Duration, change: Duration) {
        let i = self.count % 256;
        self.intake[i] = self.split.finish(i, nanos(intake));
        self.change[i] = nanos(change).saturating_add(self.queued.change);
        self.shadow[i] = self.queued.shadow;
        self.queued = QueuedTiming::default();
        self.count += 1;
    }
    #[cfg(test)]
    pub(crate) fn intake_idle(&self) -> bool {
        !self.intake_active()
    }
    pub fn intake_begin(&mut self) {
        let now = self.now();
        self.split.begin_at(now);
    }
    pub fn intake_end(&mut self) {
        let now = self.now();
        self.split.end_at(now);
    }
    /// Stop an open intake's clock for work charged to its own ring; returns whether to resume it.
    pub(crate) fn intake_suspend(&mut self) -> bool {
        let now = self.now();
        self.split.suspend_at(now)
    }
    pub(crate) fn intake_resume(&mut self) {
        let now = self.now();
        self.split.resume_at(now);
    }
    /// Queued intake and change time and the totals of the consumer and picture rings, for attribution tests.
    #[cfg(test)]
    pub(crate) fn attributed_ns(&self) -> [u64; 4] {
        let sum = |ring: &[u64; 256], count: usize| ring[..count.min(256)].iter().sum();
        [
            self.split.queued(),
            self.queued.change,
            sum(&self.consumers.samples, self.consumers.count),
            sum(&self.picture, self.picture_count),
        ]
    }
    #[cfg(test)]
    pub fn queue_intake(&mut self, duration: Duration) {
        self.split.queue(nanos(duration));
    }
    pub fn queue_change(&mut self, duration: Duration) {
        self.queued.change = self.queued.change.saturating_add(nanos(duration));
    }
    pub fn queue_shadow(&mut self, duration: Duration) {
        if !self.intake_active() {
            self.queued.shadow = self.queued.shadow.saturating_add(nanos(duration));
        }
    }
    pub fn shadow(&mut self, duration: Duration) {
        if self.count == 0 {
            self.queue_shadow(duration);
            return;
        }
        let i = (self.count - 1) % 256;
        self.shadow[i] = self.shadow[i].saturating_add(nanos(duration));
    }
    /// Record an actual main/zoom clear; idle steps do not enter this ring.
    pub fn picture(&mut self, duration: Duration) {
        self.picture[self.picture_count % 256] = nanos(duration);
        self.picture_count += 1;
    }
    pub fn consumers(&mut self, duration: Duration) {
        self.consumers.samples[self.consumers.count % 256] = nanos(duration);
        self.consumers.count += 1;
    }
    pub fn consumers_p99(&self) -> Option<u64> {
        let n = self.consumers.count.min(256);
        if n < 50 {
            return None;
        }
        let mut values = self.consumers.samples;
        values[..n].sort_unstable();
        Some(values[(n * 99).div_ceil(100) - 1])
    }
    pub fn picture_p99(&self) -> Option<u64> {
        let n = self.picture_count.min(256);
        if n < 50 {
            return None;
        }
        let mut values = self.picture;
        values[..n].sort_unstable();
        Some(values[(n * 99).div_ceil(100) - 1])
    }
    pub fn picture_max(&self) -> Option<u64> {
        self.picture[..self.picture_count.min(256)].iter().copied().max()
    }
    pub fn shadow_p99(&self) -> Option<u64> {
        let n = self.samples();
        if n < 50 {
            return None;
        }
        let mut values = self.shadow;
        values[..n].sort_unstable();
        Some(values[(n * 99).div_ceil(100) - 1])
    }
    /// Nearest-rank p99 over paired observations, not the sum of independent quantiles.
    pub fn p99(&self) -> Option<[u64; 3]> {
        let n = self.count.min(256);
        if n < 50 {
            return None;
        }
        let mut intake = self.intake;
        let mut change = self.change;
        let mut combined = std::array::from_fn::<_, 256, _>(|i| self.intake[i].saturating_add(self.change[i]));
        intake[..n].sort_unstable();
        change[..n].sort_unstable();
        combined[..n].sort_unstable();
        let rank = (n * 99).div_ceil(100) - 1;
        Some([intake[rank], change[rank], combined[rank]])
    }
    pub fn samples(&self) -> usize {
        self.count.min(256)
    }
    pub fn stage_text(&self) -> String {
        let picture = match self.picture_p99() {
            Some(p99) => format!("p99_ns={p99}"),
            None => "p99=pending".into(),
        };
        let stages = match self.p99() {
            Some([a, b, c]) => {
                format!(
                    " stage intake n={} p99_ns={} stage change n={} p99_ns={}",
                    self.samples(),
                    a,
                    self.samples(),
                    b
                ) + &format!(" stage intake+change p99_ns={c} shadow_p99_ns={}", self.shadow_p99().unwrap_or(0))
            }
            None => format!(" stage intake n={} stage change n={} p99=pending", self.samples(), self.samples()),
        };
        format!(
            "{stages} stage picture n={} {picture} max_ns={} stage consumers n={} p99_ns={}",
            self.picture_count.min(256),
            self.picture_max().unwrap_or(0),
            self.consumers.count.min(256),
            self.consumers_p99().unwrap_or(0)
        )
    }
    #[cfg(test)]
    pub fn text(&self) -> String {
        self.stage_text() + &self.diagnostics_text()
    }
    /// Appended after the existing restart fields in state queries; also used by host reports.
    pub fn diagnostics_text(&self) -> String {
        self.split.text(&self.intake, self.samples()) + &deep::counters().text() + &self.draw.text()
    }
    pub(crate) fn intake_active(&self) -> bool {
        self.split.active()
    }
    pub(crate) fn poll(&mut self, elapsed: Duration, wait: u64) {
        self.split.poll(nanos(elapsed), wait);
    }
}

fn nanos(duration: Duration) -> u64 {
    duration.as_nanos().min(u64::MAX as u128) as u64
}

#[cfg(test)]
mod stage_tests {
    use super::*;

    #[test]
    fn picture_tail_uses_only_clears_across_idle_steps_and_ring_wrap() {
        let mut timing = StageTiming::default();
        for i in 1..=400 {
            timing.add(Duration::ZERO, Duration::ZERO);
            timing.picture(Duration::from_nanos(i));
            for _ in 0..300 {
                timing.add(Duration::ZERO, Duration::ZERO);
            }
            assert_eq!(timing.picture_p99(), if i < 50 { None } else { Some(i - i.min(256) / 100) });
        }
        assert!(timing.text().contains("stage picture n=256 p99_ns=398 max_ns=400"));
    }

    #[test]
    fn picture_pending_reports_clear_count_and_max_independently() {
        let mut timing = StageTiming::default();
        for _ in 0..100 {
            timing.add(Duration::ZERO, Duration::ZERO);
        }
        assert_eq!(timing.picture_p99(), None);
        timing.picture(Duration::from_nanos(7));
        assert!(timing.text().contains("stage picture n=1 p99=pending max_ns=7"));
        for _ in 0..256 {
            timing.picture(Duration::ZERO);
        }
        assert_eq!(timing.picture_p99(), Some(0));
        assert!(timing.text().contains("stage picture n=256 p99_ns=0 max_ns=0"));
    }

    #[test]
    fn scripted_detection_is_charged_and_idle_steps_do_not_hide_consumers() {
        let mut timing = StageTiming::default();
        for _ in 0..50 {
            timing.queue_change(Duration::from_nanos(7));
            timing.add(Duration::from_nanos(10), Duration::from_nanos(13));
            timing.consumers(Duration::from_nanos(20));
        }
        assert_eq!(timing.p99(), Some([10, 20, 30]));
        assert_eq!(timing.consumers_p99(), Some(20));
        for _ in 0..300 {
            timing.add(Duration::ZERO, Duration::ZERO);
        }
        assert_eq!(timing.p99(), Some([0; 3]));
        assert_eq!(timing.consumers_p99(), Some(20));
        assert!(timing.text().contains("stage consumers n=50 p99_ns=20"));
    }

    #[test]
    fn legacy_budget_excludes_shadow_and_uses_paired_samples() {
        let mut timing = StageTiming::default();
        for _ in 0..50 {
            timing.queue_intake(Duration::from_nanos(10));
            timing.queue_shadow(Duration::from_nanos(100));
            timing.begin_change();
            timing.record_shadow(Duration::from_nanos(400));
            timing.record_shadow(Duration::from_nanos(500));
            let shadow = timing.end_change();
            let total = Duration::from_nanos(930);
            timing.add(Duration::from_nanos(20), total.saturating_sub(shadow));
            timing.shadow(shadow);
            timing.shadow(Duration::from_nanos(200));
            timing.shadow(Duration::from_nanos(300));
            timing.picture(Duration::from_nanos(2000));
        }
        assert_eq!(timing.p99(), Some([30, 30, 60]));
        assert_eq!(timing.shadow_p99(), Some(1500));
        assert_eq!(timing.picture_p99(), Some(2000));
        assert!(timing.text().contains("stage picture n=50 p99_ns=2000 max_ns=2000"));
    }
}
