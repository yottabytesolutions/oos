// SPDX-License-Identifier: GPL-2.0-or-later
//! Acceptance starts at a complete program, before decoding; idle socket time is excluded.
use crate::perfguard::{ring::Ring, Clock};
use oos_hal::sync::lock;
use std::{
    collections::VecDeque,
    sync::Mutex,
    time::{Duration, Instant},
};

pub(crate) struct Latency {
    clock: Clock,
    samples: Mutex<VecDeque<(Instant, u64)>>,
}
impl Default for Latency {
    fn default() -> Self {
        Self { clock: Clock::default(), samples: Mutex::new(VecDeque::with_capacity(256)) }
    }
}
impl Latency {
    pub(crate) fn now(&self) -> Instant {
        self.clock.now()
    }
    pub(crate) fn record(&self, accepted: Instant) {
        let now = self.now();
        let elapsed = now.saturating_duration_since(accepted).as_nanos().min(u64::MAX as u128) as u64;
        let mut samples = lock(&self.samples);
        if samples.len() == 256 {
            samples.pop_front();
        }
        samples.push_back((now, elapsed));
    }
    pub(crate) fn text(&self) -> String {
        let now = self.now();
        let mut ring = Ring::default();
        for &(at, elapsed) in lock(&self.samples).iter() {
            if now.saturating_duration_since(at) < Duration::from_secs(10) {
                ring.add(elapsed);
            }
        }
        ring.format("remote.enqueue", "ns")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn enqueue_quantiles_use_injected_time_and_expire_idle_samples() {
        let start = Instant::now();
        let now = Arc::new(Mutex::new(start));
        let clock = now.clone();
        let latency = Latency { clock: Clock::new(move || *lock(&clock)), ..Default::default() };
        assert!(latency.text().contains("n=0 total=0 p50_ns=pending p90_ns=pending p99_ns=pending"));
        for millis in 1..=100 {
            *lock(&now) = start + Duration::from_millis(millis);
            latency.record(start);
        }
        let text = latency.text();
        assert!(
            text.contains("remote.enqueue n=100 total=100 p50_ns=50000000 p90_ns=90000000 p99_ns=99000000"),
            "{text}"
        );
        *lock(&now) += Duration::from_secs(10);
        assert!(latency.text().contains("n=0 total=0 p50_ns=pending p90_ns=pending p99_ns=pending"));
    }
}
