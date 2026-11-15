// SPDX-License-Identifier: GPL-2.0-or-later

//! Best-of-rounds ratios remain the throughput guard. A separate p99 ratio checks tails.
//! Tail limits allow 50 % contention headroom over each throughput limit; they never
//! compare a candidate tail with the reference minimum. All rounds interleave the paths.

#[cfg(test)]
use crate::system::perfguard as samples;

#[cfg(test)]
pub use samples::ROUNDS;
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

/// Shared tail samples for intake/change budgets. A fixed ring avoids timing allocations in the UI.
#[derive(Clone)]
pub struct StageTiming {
    intake: [u64; 256],
    change: [u64; 256],
    count: usize,
    shadow: [u64; 256],
    queued_intake: u64,
    queued_shadow: u64,
    intake_start: Option<Instant>,
    depth: usize,
    current_shadow: Option<Duration>,
}
impl Default for StageTiming {
    fn default() -> Self {
        Self {
            intake: [0; 256],
            change: [0; 256],
            shadow: [0; 256],
            count: 0,
            queued_intake: 0,
            queued_shadow: 0,
            intake_start: None,
            depth: 0,
            current_shadow: None,
        }
    }
}
impl StageTiming {
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
        self.intake[i] = nanos(intake).saturating_add(self.queued_intake);
        self.change[i] = nanos(change);
        self.shadow[i] = self.queued_shadow;
        self.queued_intake = 0;
        self.queued_shadow = 0;
        self.count += 1;
    }
    #[cfg(test)]
    pub(crate) fn intake_idle(&self) -> bool {
        self.depth == 0 && self.intake_start.is_none()
    }
    pub fn intake_begin(&mut self) {
        if self.depth == 0 {
            self.intake_start = Some(Instant::now());
        }
        self.depth += 1;
    }
    pub fn intake_end(&mut self) {
        self.depth = self.depth.saturating_sub(1);
        if self.depth == 0 {
            if let Some(start) = self.intake_start.take() {
                self.queue_intake(start.elapsed());
            }
        }
    }
    pub fn queue_intake(&mut self, duration: Duration) {
        self.queued_intake = self.queued_intake.saturating_add(nanos(duration));
    }
    pub fn queue_shadow(&mut self, duration: Duration) {
        if self.depth == 0 {
            self.queued_shadow = self.queued_shadow.saturating_add(nanos(duration));
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
    pub fn text(&self) -> String {
        match self.p99() {
            Some([a, b, c]) => format!(
                " stage intake n={} p99_ns={} stage change n={} p99_ns={} stage intake+change p99_ns={} shadow_p99_ns={}",
                self.samples(),
                a,
                self.samples(),
                b,
                c,
                self.shadow_p99().unwrap_or(0)
            ),
            None => format!(" stage intake n={} stage change n={} p99=pending", self.samples(), self.samples()),
        }
    }
}

fn nanos(duration: Duration) -> u64 {
    duration.as_nanos().min(u64::MAX as u128) as u64
}

#[cfg(test)]
mod stage_tests {
    use super::*;

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
        }
        assert_eq!(timing.p99(), Some([30, 30, 60]));
        assert_eq!(timing.shadow_p99(), Some(1500));
    }
}
