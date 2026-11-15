// SPDX-License-Identifier: GPL-2.0-or-later

//! Host-only timing. Keep identical to the other crate copy (tested by the budget checker).
//! Each sample is one fixed-size run, with reference/candidate order reversed each round.
//! Best ratios use elapsed time; p99 uses process CPU to exclude host descheduling.
//! Quantiles are computed separately: p99(candidate) / p99(reference), never p99 of paired ratios.

use std::time::{Duration, Instant};

/// 101 samples make nearest-rank p99 the second largest, rather than the maximum of nine rounds.
pub const ROUNDS: usize = 101;

#[derive(Debug)]
pub struct Pair {
    pub best: (Duration, Duration),
    pub p99: (Duration, Duration),
}

fn summary(mut samples: Vec<Duration>) -> (Duration, Duration) {
    assert!(!samples.is_empty(), "a timing distribution needs samples");
    samples.sort_unstable();
    (samples[0], samples[(99 * samples.len()).div_ceil(100) - 1])
}

fn collect(rounds: usize, mut time: impl FnMut(bool) -> (Duration, Duration)) -> Pair {
    assert!(rounds > 0, "a comparison needs rounds");
    let (mut reference, mut candidate) = (Vec::with_capacity(rounds), Vec::with_capacity(rounds));
    for round in 0..rounds {
        if round % 2 == 0 {
            reference.push(time(false));
            candidate.push(time(true));
        } else {
            candidate.push(time(true));
            reference.push(time(false));
        }
    }
    let elapsed = |v: &[(Duration, Duration)]| summary(v.iter().map(|x| x.0).collect()).0;
    let cpu = |v: &[(Duration, Duration)]| summary(v.iter().map(|x| x.1).collect()).1;
    Pair { best: (elapsed(&reference), elapsed(&candidate)), p99: (cpu(&reference), cpu(&candidate)) }
}

pub fn measure(rounds: usize, runs: usize, mut reference: impl FnMut(), mut candidate: impl FnMut()) -> Pair {
    assert!(runs > 0 && u32::try_from(runs).is_ok(), "runs must fit a nonzero u32");
    collect(rounds, |is_candidate| {
        let f: &mut dyn FnMut() = if is_candidate { &mut candidate } else { &mut reference };
        let cpu = process_cpu();
        let start = Instant::now();
        for _ in 0..runs {
            f();
        }
        let elapsed = start.elapsed();
        (elapsed / runs as u32, (process_cpu() - cpu) / runs as u32)
    })
}

/// POSIX process CPU includes asynchronous workers, excludes time when the host deschedules us.
/// This test adapter lives under system (UI) or in HAL tests, the permitted unsafe boundaries.
fn process_cpu() -> Duration {
    // SAFETY: rusage is a plain C result record; zero is a valid initial value for every field.
    let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
    // SAFETY: usage is writable for the complete rusage record; RUSAGE_SELF queries this process only.
    let status = unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut usage) };
    assert_eq!(status, 0, "the host must provide process CPU accounting");
    let time = |v: libc::timeval| Duration::from_secs(v.tv_sec as u64) + Duration::from_micros(v.tv_usec as u64);
    time(usage.ru_utime) + time(usage.ru_stime)
}

impl Pair {
    pub fn report(&self, id: &str) {
        eprintln!(
            "PERF_BUDGET {id} {} {} {} {}",
            self.best.0.as_nanos(),
            self.best.1.as_nanos(),
            self.p99.0.as_nanos(),
            self.p99.1.as_nanos()
        );
    }

    pub fn check_tail(&self, limit: f64) {
        let ratio = self.p99.1.as_secs_f64() / self.p99.0.as_secs_f64();
        eprintln!(
            "CPU tail: reference p99 {:?}, candidate p99 {:?}, ratio {:.3}, limit {:.3}",
            self.p99.0, self.p99.1, ratio, limit
        );
        assert!(ratio <= limit, "candidate/reference p99 ratio {ratio:.3} exceeds {limit:.3}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_rank_uses_the_whole_distribution() {
        let times = (1..=101).rev().map(Duration::from_nanos).collect();
        assert_eq!(summary(times), (Duration::from_nanos(1), Duration::from_nanos(100)));
        assert_eq!(summary(vec![Duration::from_nanos(7)]), (Duration::from_nanos(7), Duration::from_nanos(7)));
    }

    #[test]
    fn shared_host_load_and_alternating_order() {
        let mut order = Vec::new();
        let pair = collect(ROUNDS, |candidate| {
            let round = order.len() / 2;
            order.push(candidate);
            // A deterministic 100x host slowdown hits both adjacent sides in the same rounds.
            let cpu = Duration::from_nanos(1000 * if candidate { 2 } else { 4 });
            let elapsed = cpu * if round % 7 == 0 { 100 } else { 1 };
            (elapsed, cpu)
        });
        assert_eq!(&order[..8], &[false, true, true, false, false, true, true, false]);
        assert_eq!(pair.best.0, pair.best.1 * 2);
        assert_eq!(pair.p99.0, pair.p99.1 * 2);
        pair.check_tail(0.6);
    }

    #[test]
    #[should_panic(expected = "candidate/reference p99 ratio")]
    fn a_candidate_tail_regression_fails_even_with_a_fast_best() {
        let mut calls = 0;
        let pair = collect(ROUNDS, |candidate| {
            let round = calls / 2;
            calls += 1;
            let time = Duration::from_nanos(if candidate && round % 10 == 0 { 5000 } else { 1000 });
            (time, time)
        });
        assert_eq!(pair.best.0, pair.best.1);
        pair.check_tail(1.5);
    }

    #[test]
    fn runs_and_rounds_are_not_silently_changed() {
        let (mut a, mut b) = (0, 0);
        measure(4, 3, || a += 1, || b += 1);
        assert_eq!((a, b), (12, 12));
    }
}
