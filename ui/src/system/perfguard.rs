// SPDX-License-Identifier: GPL-2.0-or-later

//! Host-only timing. Keep identical to the other crate copy (tested by the budget checker).
//! Each sample is one fixed-size run, with reference/candidate order reversed each round.
//! Both ratios use CPU time. Worker comparisons account for all threads in the serial test process.
//! Quantiles are computed separately: p99(candidate) / p99(reference), never p99 of paired ratios.

use std::time::Duration;

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

fn collect(rounds: usize, runs: u32, matched: bool, mut time: impl FnMut(bool, u32) -> Duration) -> Pair {
    assert!(rounds > 0, "a comparison needs rounds");
    let (mut reference, mut candidate) = (Vec::with_capacity(rounds), Vec::with_capacity(rounds));
    let (pairs, iterations) = if matched { (runs, 1) } else { (1, runs) };
    for round in 0..rounds {
        let mut times = [Duration::ZERO; 2];
        for pair in 0..pairs {
            let first = (round + pair as usize) % 2;
            times[first] += time(first == 1, iterations);
            times[1 - first] += time(first == 0, iterations);
        }
        reference.push(times[0] / runs);
        candidate.push(times[1] / runs);
    }
    let paired = matched.then(|| matched_best(&reference, &candidate));
    let (ref_best, ref_p99) = summary(reference);
    let (new_best, new_p99) = summary(candidate);
    Pair { best: paired.unwrap_or((ref_best, new_best)), p99: (ref_p99, new_p99) }
}

// Independent minima can pair different CPU scheduling regimes in a threaded fixture.
// Minimize total CPU over matched windows that cover two thirds of the run. A full
// window captures repeated work across dispatches and preserves both sides of each pair.
fn matched_best(reference: &[Duration], candidate: &[Duration]) -> (Duration, Duration) {
    let width = (2 * reference.len()).div_ceil(3);
    let total = |times: &[Duration]| times.iter().sum::<Duration>();
    let (a, b) = reference
        .windows(width)
        .zip(candidate.windows(width))
        .map(|(a, b)| (total(a), total(b)))
        .min_by_key(|&(a, b)| a + b)
        .expect("a sampled run has at least one matched window");
    (a / width as u32, b / width as u32)
}

pub fn measure(rounds: usize, runs: usize, reference: impl FnMut(), candidate: impl FnMut()) -> Pair {
    measure_cpu(thread_cpu, false, rounds, runs, reference, candidate)
}

/// Only for comparisons that await work on other threads. The serial guard binary has
/// no other test running, so this accounts for worker and caller CPU without queue wait.
pub fn measure_workers(rounds: usize, runs: usize, reference: impl FnMut(), candidate: impl FnMut()) -> Pair {
    measure_cpu(process_cpu, false, rounds, runs, reference, candidate)
}

/// Alternate the paths after each picture on the same workers, then compare the fastest
/// matched window. Long uninterrupted runs otherwise give each path its own scheduling period.
pub fn measure_matched_workers(rounds: usize, runs: usize, reference: impl FnMut(), candidate: impl FnMut()) -> Pair {
    measure_cpu(process_cpu, true, rounds, runs, reference, candidate)
}

fn process_cpu() -> Duration {
    cpu_time(libc::CLOCK_PROCESS_CPUTIME_ID)
}

fn measure_cpu(
    now: impl Fn() -> Duration,
    matched: bool,
    rounds: usize,
    runs: usize,
    mut reference: impl FnMut(),
    mut candidate: impl FnMut(),
) -> Pair {
    let count = u32::try_from(runs).expect("sample run count fits u32");
    assert!(count > 0, "a sample needs at least one run");
    collect(rounds, count, matched, |is_candidate, iterations| {
        let f: &mut dyn FnMut() = if is_candidate { &mut candidate } else { &mut reference };
        let start = now();
        for _ in 0..iterations {
            f();
        }
        now() - start
    })
}

fn thread_cpu() -> Duration {
    cpu_time(libc::CLOCK_THREAD_CPUTIME_ID)
}

fn cpu_time(clock: libc::clockid_t) -> Duration {
    let mut time = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    // SAFETY: time points to a writable timespec, and callers supply a valid CPU clock ID.
    let status = unsafe { libc::clock_gettime(clock, &mut time) };
    assert_eq!(status, 0, "the host must provide POSIX CPU accounting");
    Duration::new(time.tv_sec as u64, time.tv_nsec as u32)
}

impl Pair {
    pub fn report(&self, id: &str) {
        eprintln!(
            "CPU_BUDGET {id} {} {} {} {}",
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
    fn reference_and_candidate_alternate_in_each_round() {
        let mut order = Vec::new();
        let pair = collect(ROUNDS, 1, false, |candidate, _| {
            order.push(candidate);
            Duration::from_nanos(if candidate { 2000 } else { 4000 })
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
        let pair = collect(ROUNDS, 1, false, |candidate, _| {
            let round = calls / 2;
            calls += 1;
            Duration::from_nanos(if candidate && round % 10 == 0 { 5000 } else { 1000 })
        });
        assert_eq!(pair.best.0, pair.best.1);
        pair.check_tail(1.5);
    }

    #[test]
    fn matched_rounds_switch_after_each_picture() {
        let mut order = Vec::new();
        let pair = collect(2, 3, true, |candidate, runs| {
            order.push((candidate, runs));
            Duration::from_nanos(if candidate { 5 } else { 4 })
        });
        assert_eq!(
            order,
            [
                (false, 1),
                (true, 1),
                (true, 1),
                (false, 1),
                (false, 1),
                (true, 1),
                (true, 1),
                (false, 1),
                (false, 1),
                (true, 1),
                (true, 1),
                (false, 1),
            ]
        );
        assert_eq!(pair.best, (Duration::from_nanos(4), Duration::from_nanos(5)));
    }

    #[test]
    fn minimum_cost_matched_pairs_require_comparable_rounds() {
        let samples = |times: [u64; 12]| times.map(Duration::from_nanos);
        let reference = samples([500, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1950, 1950, 1950]);
        let mut candidate = samples([2000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1950, 1950, 1950]);
        assert_eq!(matched_best(&reference, &candidate), (Duration::from_nanos(1000), Duration::from_nanos(1000)));
        for duration in &mut candidate[1..9] {
            *duration += Duration::from_nanos(30);
        }
        assert_eq!(matched_best(&reference, &candidate).1, Duration::from_nanos(1030));
    }

    #[test]
    fn runs_and_rounds_are_not_silently_changed() {
        let (mut a, mut b) = (0, 0);
        measure(4, 3, || a += 1, || b += 1);
        measure_workers(4, 3, || a += 1, || b += 1);
        measure_matched_workers(4, 3, || a += 1, || b += 1);
        assert_eq!((a, b), (36, 36));
    }
}
