// SPDX-License-Identifier: GPL-2.0-or-later
//! Monotonic RSS policy. Samples are supplied by the process adapter or a test.

use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Limits {
    soft: u64,
    hard: u64,
}

impl Limits {
    pub fn parse(conf: &str) -> Self {
        let mb = |key, default| {
            crate::sysconf::conf_value(None, conf, key)
                .and_then(|s| s.parse::<u64>().ok())
                .filter(|n| *n > 0 && *n <= 4096)
                .unwrap_or(default)
                << 20
        };
        let hard = mb("mem.hard_mb", 1024);
        Self { soft: mb("mem.soft_mb", 768).min(hard), hard }
    }
}

#[derive(Debug, PartialEq)]
pub enum Action {
    None,
    Warn,
    Stop,
    Exit75,
}

pub struct Watchdog {
    limits: Limits,
    next: Duration,
    recovery: Option<Duration>,
    warned: bool,
    stopped: bool,
    rss: Option<u64>,
    exit: bool,
}

impl Watchdog {
    pub fn new(limits: Limits) -> Self {
        Self { limits, next: Duration::ZERO, recovery: None, warned: false, stopped: false, rss: None, exit: false }
    }

    pub fn due(&self, now: Duration) -> bool {
        now >= self.next
    }
    pub fn stopped(&self) -> bool {
        self.stopped
    }
    pub fn exit_code(&self) -> i32 {
        if self.exit {
            75
        } else {
            0
        }
    }

    pub fn sample(&mut self, now: Duration, rss: Option<u64>) -> Action {
        if !self.due(now) {
            return Action::None;
        }
        self.next = now + Duration::from_secs(1);
        self.rss = rss;
        let Some(rss) = rss else {
            if self.recovery.is_some_and(|since| now.saturating_sub(since) >= Duration::from_secs(5)) {
                self.exit = true;
                return Action::Exit75;
            }
            return Action::None;
        };
        if rss > self.limits.hard {
            if let Some(since) = self.recovery {
                if now.saturating_sub(since) >= Duration::from_secs(5) {
                    self.exit = true;
                    return Action::Exit75;
                }
            } else {
                self.recovery = Some(now);
                self.stopped = true;
                return Action::Stop;
            }
            return Action::None;
        } else {
            self.recovery = None;
        }
        if rss > self.limits.soft && !self.warned {
            self.warned = true;
            return Action::Warn;
        }
        self.warned = rss > self.limits.soft;
        Action::None
    }

    pub fn resume(&mut self) {
        if self.recovery.is_none() {
            self.stopped = false;
        }
    }

    pub fn status(&self) -> String {
        format!(
            "rss {} watchdog {}",
            self.rss.map_or("unknown".into(), |n| n.to_string()),
            if self.exit {
                "exit75"
            } else if self.recovery.is_some() {
                "recovering"
            } else if self.stopped {
                "stopped"
            } else if self.warned {
                "soft"
            } else {
                "ok"
            }
        )
    }
}

pub fn rss(status: &str) -> Option<u64> {
    let mut fields = status.lines().find(|s| s.starts_with("VmRSS:"))?.split_whitespace();
    fields.next()?;
    let kb: u64 = fields.next()?.parse().ok()?;
    (fields.next()? == "kB").then(|| kb.checked_mul(1024)).flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hard_limit_stops_then_exits_75() {
        let mut w = Watchdog::new(Limits { soft: 100, hard: 200 });
        assert_eq!(w.sample(Duration::ZERO, Some(201)), Action::Stop);
        for s in 1..5 {
            assert_eq!(w.sample(Duration::from_secs(s), Some(201)), Action::None);
        }
        assert_eq!(w.sample(Duration::from_secs(5), Some(201)), Action::Exit75);
        assert_eq!(w.exit_code(), 75);
    }

    #[test]
    fn recovery_stays_stopped_and_a_later_runaway_gets_a_new_deadline() {
        let mut w = Watchdog::new(Limits { soft: 100, hard: 200 });
        assert_eq!(w.sample(Duration::ZERO, Some(201)), Action::Stop);
        assert_eq!(w.sample(Duration::from_secs(1), Some(50)), Action::None);
        assert!(w.stopped());
        w.resume();
        assert!(!w.stopped());
        assert_eq!(w.sample(Duration::from_secs(5), Some(201)), Action::Stop);
        assert_eq!(w.sample(Duration::from_secs(9), Some(201)), Action::None);
        assert_eq!(w.sample(Duration::from_secs(10), Some(201)), Action::Exit75);
    }

    #[test]
    fn warnings_are_once_per_crossing_and_samples_once_per_second() {
        let mut w = Watchdog::new(Limits { soft: 100, hard: 200 });
        assert_eq!(w.sample(Duration::ZERO, Some(101)), Action::Warn);
        assert_eq!(w.sample(Duration::from_millis(999), Some(300)), Action::None);
        assert_eq!(w.sample(Duration::from_secs(1), Some(150)), Action::None);
        assert_eq!(w.sample(Duration::from_secs(2), None), Action::None);
        assert_eq!(w.sample(Duration::from_secs(3), Some(50)), Action::None);
        assert_eq!(w.sample(Duration::from_secs(4), Some(101)), Action::Warn);
        assert_eq!(rss("Name: ui\nVmRSS:\t123 kB\n"), Some(123 * 1024));
        assert_eq!(rss("VmRSS: 99 bytes"), None);
    }

    #[test]
    fn missing_recovery_sample_does_not_clear_the_hard_limit() {
        let mut w = Watchdog::new(Limits { soft: 100, hard: 200 });
        assert_eq!(w.sample(Duration::ZERO, Some(201)), Action::Stop);
        assert_eq!(w.sample(Duration::from_secs(1), None), Action::None);
        assert_eq!(w.sample(Duration::from_secs(5), None), Action::Exit75);
    }
}
