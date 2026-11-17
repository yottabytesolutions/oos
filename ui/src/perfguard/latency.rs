// SPDX-License-Identifier: GPL-2.0-or-later
//! Real duration budgets run only in the explicitly selected release performance lane.
use std::time::{Duration, Instant};

pub(crate) fn check(limit: Duration, action: impl FnOnce()) {
    assert_eq!(std::env::var("OOS_PERF_TESTS").as_deref(), Ok("1"));
    if cfg!(debug_assertions) {
        panic!("latency evidence requires release");
    }
    let start = Instant::now();
    action();
    let elapsed = start.elapsed();
    println!("latency: {elapsed:?}, limit {limit:?}");
    assert!(elapsed < limit, "latency {elapsed:?} exceeds {limit:?}");
}

pub(crate) fn wait_until(deadline: Instant) {
    std::thread::sleep(deadline.saturating_duration_since(Instant::now()));
}
