// SPDX-License-Identifier: GPL-2.0-or-later

//! Best-of-rounds ratios remain the throughput guard. A separate p99 ratio checks tails.
//! Tail limits allow 50 % contention headroom over each throughput limit; they never
//! compare a candidate tail with the reference minimum. All rounds interleave the paths.

use crate::system::perfguard as samples;

pub use samples::ROUNDS;
use std::time::Duration;

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
