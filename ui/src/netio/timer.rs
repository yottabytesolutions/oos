// SPDX-License-Identifier: GPL-2.0-or-later
use std::time::Duration;

pub(super) struct SendTimer {
    started: Duration,
    progress: Duration,
}
impl SendTimer {
    pub(super) fn new(now: Duration) -> Self {
        Self { started: now, progress: now }
    }
    pub(super) fn progress(&mut self, now: Duration) {
        self.progress = now;
    }
    pub(super) fn deadline(&self) -> Duration {
        (self.started + Duration::from_secs(30)).min(self.progress + Duration::from_secs(2))
    }
    pub(super) fn expired(&self, now: Duration) -> bool {
        now >= self.deadline()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn injected_time_distinguishes_stall_from_total_limit() {
        let mut timer = SendTimer::new(Duration::ZERO);
        assert!(!timer.expired(Duration::from_millis(1999)));
        assert!(timer.expired(Duration::from_secs(2)));
        for second in 1..30 {
            assert!(!timer.expired(Duration::from_secs(second)));
            timer.progress(Duration::from_secs(second));
        }
        assert_eq!(timer.deadline(), Duration::from_secs(30));
        assert!(timer.expired(Duration::from_secs(30)));
    }
}
