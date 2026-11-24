// SPDX-License-Identifier: GPL-2.0-or-later
//! Bounded helper restart schedule; CPU presentation continues during backoff.

use std::time::Duration;

#[derive(Default)]
pub struct Recovery {
    failures: u64,
    attempts: usize,
    restarts: u64,
    retry: Option<Duration>,
    stable_since: Option<Duration>,
}

impl Recovery {
    pub fn failed(&mut self, now: Duration) {
        self.failures += 1;
        self.stable_since = None;
        self.retry = [1, 5, 30].get(self.attempts).map(|s| now + Duration::from_secs(*s));
    }

    pub fn ready(&mut self, now: Duration) -> bool {
        if self.retry.is_some_and(|at| now >= at) {
            self.retry = None;
            self.attempts += 1;
            self.restarts += 1;
            return true;
        }
        false
    }

    /// Acknowledged pictures must span ten seconds before a restart restores its failure budget.
    pub fn succeeded(&mut self, now: Duration) {
        let since = *self.stable_since.get_or_insert(now);
        if now.saturating_sub(since) >= Duration::from_secs(10) {
            self.attempts = 0;
            self.retry = None;
        }
    }

    pub fn status(&self) -> String {
        format!(
            "gpu_failures {} gpu_restarts {} gpu_retry {}",
            self.failures,
            self.restarts,
            if self.retry.is_some() {
                "waiting"
            } else if self.attempts >= 3 {
                "exhausted"
            } else {
                "idle"
            }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_failed_restarts_stay_on_cpu() {
        let mut r = Recovery::default();
        r.failed(Duration::ZERO);
        for (before, due) in [(0, 1), (5, 6), (35, 36)] {
            assert!(!r.ready(Duration::from_secs(before)));
            assert!(r.ready(Duration::from_secs(due)));
            r.failed(Duration::from_secs(due));
        }
        assert!(!r.ready(Duration::from_secs(10000)));
        assert!(r.status().contains("gpu_failures 4 gpu_restarts 3 gpu_retry exhausted"));
    }

    #[test]
    fn one_picture_does_not_reset_repeated_short_lived_helpers() {
        let mut r = Recovery::default();
        r.failed(Duration::ZERO);
        for due in [1, 7, 38] {
            assert!(r.ready(Duration::from_secs(due)));
            r.succeeded(Duration::from_secs(due));
            r.failed(Duration::from_secs(due + 1));
            assert!(!r.ready(Duration::from_secs(due + 2)));
        }
        assert!(!r.ready(Duration::from_secs(10000)));
    }

    #[test]
    fn ten_seconds_of_acknowledged_pictures_restores_budget_and_keeps_counters() {
        let mut r = Recovery::default();
        r.failed(Duration::ZERO);
        assert!(r.ready(Duration::from_secs(1)));
        r.succeeded(Duration::from_secs(1));
        r.succeeded(Duration::from_secs(11));
        r.failed(Duration::from_secs(12));
        assert!(r.ready(Duration::from_secs(13)));
        assert!(r.status().contains("gpu_failures 2 gpu_restarts 2"));
        r.failed(Duration::from_secs(13));
        assert!(!r.ready(Duration::from_secs(17)));
        assert!(r.ready(Duration::from_secs(18)));
    }
}
