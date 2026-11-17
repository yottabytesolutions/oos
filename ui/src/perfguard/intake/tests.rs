// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::perfguard::StageTiming;
use std::time::Duration;

#[test]
fn every_split_sums_to_its_paired_intake_through_wrap_and_saturation() {
    let mut timing = StageTiming::default();
    for i in 1..=400 {
        // External intake can contain polls. Here 10 ns are queued external body time.
        timing.split.queue(10);
        timing.split.pending[1] = 10;
        timing.poll(Duration::from_nanos(i / 2), i / 4);
        timing.add(Duration::from_nanos(i), Duration::from_nanos(1000 - i));
        let index = (timing.count - 1) % 256;
        assert_eq!(timing.split.samples[index].iter().sum::<u64>(), timing.intake[index]);
        assert_eq!(timing.split.samples[index], [i / 2 - i / 4, 10, i / 4, i - i / 2]);
    }
    let text = timing.diagnostics_text();
    assert!(
        text.contains("diag intake.p50 n=256 intake_ns=282 polls_ns=68 external_ns=10 lock_wait_ns=68 residual_ns=136"),
        "{text}"
    );
    assert!(
        text.contains(
            "diag intake.p99 n=256 intake_ns=408 polls_ns=100 external_ns=10 lock_wait_ns=99 residual_ns=199"
        ),
        "{text}"
    );
    assert_eq!(timing.p99().unwrap()[0], 408);
    timing.split.pending = [u64::MAX; 3];
    timing.split.queue(u64::MAX);
    timing.add(Duration::from_nanos(1), Duration::ZERO);
    assert_eq!(timing.split.samples[400 % 256], [u64::MAX, 0, 0, 0]);
    timing.add(Duration::from_nanos(7), Duration::ZERO);
    assert_eq!(timing.split.samples[401 % 256], [0, 0, 0, 7]);
}

#[test]
fn rank_selects_one_samples_components_instead_of_independent_quantiles() {
    let mut timing = StageTiming::default();
    assert!(timing.diagnostics_text().contains("diag intake.p50 n=0 pending diag intake.p99 n=0 pending"));
    for i in (1..=100).rev() {
        timing.split.pending = if i == 50 {
            [1, 2, 3]
        } else if i == 99 {
            [4, 5, 6]
        } else {
            [i, 0, 0]
        };
        timing.add(Duration::from_nanos(i), Duration::ZERO);
        if i == 100 {
            assert!(timing.diagnostics_text().contains("diag intake.p99 n=1 pending"));
        }
    }
    let text = timing.diagnostics_text();
    assert!(text.contains("diag intake.p50 n=100 intake_ns=50 polls_ns=1 external_ns=2 lock_wait_ns=3 residual_ns=44"));
    assert!(text.contains("diag intake.p99 n=100 intake_ns=99 polls_ns=4 external_ns=5 lock_wait_ns=6 residual_ns=84"));
}

#[test]
fn external_nesting_excludes_its_polls_and_their_waits_once() {
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let mut timing = Timing::default();
    let now = Instant::now();
    timing.begin_at(now);
    timing.begin_at(now + Duration::from_nanos(10));
    // A synthetic 100 ns poll with 40 ns Shared wait occurs inside outer intake.
    timing.poll(100, 40);
    deep::counters().locked_for_test(40);
    timing.end_at(now + Duration::from_nanos(200));
    assert_eq!(timing.queued, 0);
    timing.end_at(now + Duration::from_nanos(500));
    let queued = timing.queued;
    assert_eq!(queued, 500);
    let total = timing.finish(0, 7);
    let [polls, external, wait, residual] = timing.samples[0];
    assert_eq!([polls, wait, residual], [60, 40, 7]);
    assert_eq!(external, queued - 100);
    assert_eq!(polls + external + wait + residual, total);
    assert!(deep::counters().text().contains(&format!("poll.external n=1 total=1 p50_ns={queued}")));
}
