// SPDX-License-Identifier: GPL-2.0-or-later

//! Host-only timing. Keep identical to the other crate copy (tested by the budget checker).
//! Each sample is one fixed-size run, with reference/candidate order reversed each round.
//! Worker comparisons account for all threads in the serial test process.
//! Quantiles are computed separately: p99(candidate) / p99(reference), never p99 of paired ratios.

use std::time::Duration;

/// 101 samples make nearest-rank p99 the second largest, rather than the maximum of nine rounds.
pub const ROUNDS: usize = 101;

#[derive(Debug)]
pub struct Pair {
    pub best: (Duration, Duration),
    pub p99: (Duration, Duration),
    instructions: Option<(u64, u64)>,
}

#[derive(Clone, Copy, Default)]
struct Sample {
    cpu: Duration,
    instructions: u64,
}

impl Sample {
    fn add(&mut self, other: Self) {
        self.cpu += other.cpu;
        self.instructions += other.instructions;
    }
}

impl From<Duration> for Sample {
    fn from(cpu: Duration) -> Self {
        Self { cpu, instructions: 0 }
    }
}

fn summary(mut samples: Vec<Duration>) -> (Duration, Duration) {
    assert!(!samples.is_empty(), "a timing distribution needs samples");
    samples.sort_unstable();
    (samples[0], samples[(99 * samples.len()).div_ceil(100) - 1])
}

fn collect(rounds: usize, runs: u32, matched: bool, mut observe: impl FnMut(bool, u32) -> Sample) -> Pair {
    assert!(rounds > 0, "a comparison needs rounds");
    let (mut reference, mut candidate) = (Vec::with_capacity(rounds), Vec::with_capacity(rounds));
    let mut instructions = Vec::new();
    let (pairs, iterations) = if matched { (runs, 1) } else { (1, runs) };
    for round in 0..rounds {
        let mut samples = [Sample::default(); 2];
        for pair in 0..pairs {
            let first = (round + pair as usize) % 2;
            samples[first].add(observe(first == 1, iterations));
            samples[1 - first].add(observe(first == 0, iterations));
        }
        reference.push(samples[0].cpu / runs);
        candidate.push(samples[1].cpu / runs);
        if samples[0].instructions > 0 {
            instructions.push((samples[0].instructions, samples[1].instructions));
        }
    }
    let instructions = (!instructions.is_empty()).then(|| median_instruction_pair(&mut instructions));
    let paired = (matched && instructions.is_none()).then(|| matched_best(&reference, &candidate));
    let (ref_best, ref_p99) = summary(reference);
    let (new_best, new_p99) = summary(candidate);
    Pair { instructions, ..Pair::cpu(paired.unwrap_or((ref_best, new_best)), (ref_p99, new_p99)) }
}

fn median_instruction_pair(pairs: &mut [(u64, u64)]) -> (u64, u64) {
    let ratio = |&(a, b): &(u64, u64)| b as f64 / a as f64;
    let middle = pairs.len() / 2;
    let (_, median, _) = pairs.select_nth_unstable_by(middle, |a, b| ratio(a).total_cmp(&ratio(b)));
    *median
}

// Scheduler and core-placement noise can add CPU cost to either side. Rank whole
// pairs by combined cost to find quiet rounds, even when they are scattered.
// Take the median ratio within the fastest quarter so one cheap but skewed pair
// cannot hide a regression. Retain both times from the selected pair for reporting.
fn matched_best(reference: &[Duration], candidate: &[Duration]) -> (Duration, Duration) {
    let mut pairs: Vec<_> = reference.iter().copied().zip(candidate.iter().copied()).collect();
    pairs.sort_by_key(|&(a, b)| a + b);
    pairs.truncate(pairs.len().div_ceil(4));
    let ratio = |&(a, b): &(Duration, Duration)| b.as_secs_f64() / a.as_secs_f64();
    let middle = pairs.len() / 2;
    let (_, median, _) = pairs.select_nth_unstable_by(middle, |a, b| ratio(a).total_cmp(&ratio(b)));
    *median
}

pub fn measure(rounds: usize, runs: usize, reference: impl FnMut(), candidate: impl FnMut()) -> Pair {
    measure_cpu(thread_cpu, false, rounds, runs, reference, candidate)
}

/// Only for comparisons that await work on other threads. The serial guard binary has
/// no other test running, so this accounts for worker and caller CPU without queue wait.
pub fn measure_workers(rounds: usize, runs: usize, reference: impl FnMut(), candidate: impl FnMut()) -> Pair {
    measure_cpu(process_cpu, false, rounds, runs, reference, candidate)
}

/// Alternate the paths after each picture on the same workers and count their retired instructions
/// when the host exposes them. CPU p99 always measures all rounds.
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
    let count_instructions = matched && process_instructions() != 0;
    collect(rounds, count, matched, |is_candidate, iterations| {
        let f: &mut dyn FnMut() = if is_candidate { &mut candidate } else { &mut reference };
        let first = if count_instructions { process_instructions() } else { 0 };
        let start = now();
        for _ in 0..iterations {
            f();
        }
        let cpu = now() - start;
        let instructions = if count_instructions {
            let last = process_instructions();
            assert!(last > first, "the process instruction counter must advance");
            last - first
        } else {
            0
        };
        Sample { cpu, instructions }
    })
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn process_instructions() -> u64 {
    let mut usage = std::mem::MaybeUninit::<libc::rusage_info_v4>::uninit();
    // SAFETY: RUSAGE_INFO_V4 requests the exact layout allocated for the current process.
    let status = unsafe {
        libc::proc_pid_rusage(std::process::id() as libc::pid_t, libc::RUSAGE_INFO_V4, usage.as_mut_ptr().cast())
    };
    if status != 0 {
        return 0;
    }
    // SAFETY: a successful call initializes the requested rusage structure.
    unsafe { usage.assume_init() }.ri_instructions
}

#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
fn process_instructions() -> u64 {
    0
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
    pub(crate) fn cpu(best: (Duration, Duration), p99: (Duration, Duration)) -> Self {
        Self { best, p99, instructions: None }
    }

    pub fn matched_ratio(&self) -> f64 {
        self.instructions
            .map_or_else(|| self.best.1.as_secs_f64() / self.best.0.as_secs_f64(), |(a, b)| b as f64 / a as f64)
    }

    pub fn report(&self, id: &str) {
        let (marker, a, b) =
            self.instructions.map_or(("CPU_BUDGET", self.best.0.as_nanos(), self.best.1.as_nanos()), |(a, b)| {
                ("INSTRUCTION_BUDGET", u128::from(a), u128::from(b))
            });
        eprintln!("{marker} {id} {a} {b} {} {}", self.p99.0.as_nanos(), self.p99.1.as_nanos());
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
            Duration::from_nanos(if candidate { 2000 } else { 4000 }).into()
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
            Duration::from_nanos(if candidate && round % 10 == 0 { 5000 } else { 1000 }).into()
        });
        assert_eq!(pair.best.0, pair.best.1);
        pair.check_tail(1.5);
    }

    #[test]
    fn matched_rounds_switch_after_each_picture() {
        let mut order = Vec::new();
        let pair = collect(2, 3, true, |candidate, runs| {
            order.push((candidate, runs));
            Duration::from_nanos(if candidate { 5 } else { 4 }).into()
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
    fn matched_cost_rejects_a_regression_between_scattered_noisy_rounds() {
        let measure = |added: u64| {
            let mut calls = 0;
            collect(20, 2, true, |is_candidate, _| {
                let round = calls / 4;
                calls += 1;
                let nanos = match (round % 4, is_candidate) {
                    (0, true) if round == 0 => 950,
                    (0, true) => 1000 + added,
                    (1 | 2, true) | (3, false) => 1200,
                    _ => 1000,
                };
                Duration::from_nanos(nanos).into()
            })
        };
        assert_eq!(measure(0).best, (Duration::from_nanos(1000), Duration::from_nanos(1000)));
        assert_eq!(measure(20).best, (Duration::from_nanos(1000), Duration::from_nanos(1020)));
    }

    #[test]
    fn matched_instructions_detect_work_even_when_candidate_cpu_is_lower() {
        let mut calls = 0;
        let pair = collect(9, 2, true, |is_candidate, _| {
            let round = calls / 4;
            calls += 1;
            let instructions = match (is_candidate, round) {
                (true, 0) => 900,
                (true, 8) => 1100,
                (true, _) => 1020,
                (false, _) => 1000,
            };
            Sample { cpu: Duration::from_nanos(if is_candidate { 500 } else { 1000 }), instructions }
        });
        assert_eq!(pair.matched_ratio(), 1.02);
        assert_eq!(pair.p99, (Duration::from_nanos(1000), Duration::from_nanos(500)));
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
