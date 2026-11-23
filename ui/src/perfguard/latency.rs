// SPDX-License-Identifier: GPL-2.0-or-later
//! Wall-time check for the manually selected local waveform benchmark.
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
