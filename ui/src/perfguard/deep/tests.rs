// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use std::time::Duration;

#[test]
fn guard_records_both_sides_and_releases_shared_before_recording() {
    let diagnostics = Diagnostics::default();
    let shared = Mutex::new(0);
    let samples = lock(&diagnostics.locks[0]);
    std::thread::scope(|scope| {
        let (tx, rx) = std::sync::mpsc::channel();
        let shared = &shared;
        let diagnostics = &diagnostics;
        scope.spawn(move || {
            let guard = timed_lock(shared, diagnostics, 0);
            tx.send(()).unwrap();
            drop(guard);
        });
        rx.recv().unwrap();
        // Drop can block on the metric mutex, but must first release the application mutex.
        // Taking Shared is the event: the worker must release it before waiting on samples.
        drop(crate::ui::lifecycle::test_wait::until("application mutex released before metrics", || {
            shared.try_lock().ok()
        }));
        drop(samples);
    });
    *timed_lock(&shared, &diagnostics, 1) += 1;
    let text = diagnostics.text();
    for name in ["ui.wait", "ui.held", "acq.wait", "acq.held"] {
        assert!(text.contains(&format!("diag lock.{name} n=1 total=1 p50_ns=")));
    }
    assert_eq!(*lock(&shared), 1);
}

#[test]
fn polls_external_nesting_and_state_query_share_the_perf_report() {
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let mut ui = crate::ui::Ui::for_tests(crate::wave::Source::Hw { shared: Default::default(), seen: 0 });
    crate::ui::change::intake(&mut ui, |ui| crate::ui::change::intake(ui, |_| {}));
    ui.step(0.04);
    let expected = counters().text();
    let stage = super::super::StageTiming::default().text();
    assert!(stage.contains(&expected));
    let text = crate::cmd::stat_text(&ui);
    for poll in &Poll::ALL[..16] {
        let poll = poll.label();
        assert!(text.contains(&format!("diag poll.{poll} n=1 total=1")), "{poll}: {text}");
    }
    assert!(text.contains("diag poll.external n=1 total=1"));
    assert!(text.contains(&expected));
    let restart = text.find(" stage_restart_bode=").unwrap();
    let diagnostic = text.find(" diag ").unwrap();
    assert!(restart < diagnostic);
    assert!(text.contains("diag intake.p50 n=1 intake_ns="));
    assert!(text.contains("diag lock.ui.wait n="));
    assert!(text.contains("diag lock.acq.wait n=0 total=0 p50_ns=pending p90_ns=pending p99_ns=pending max_ns=0"));
    assert!(std::mem::size_of::<Diagnostics>() <= 56 * 1024);
}

#[test]
fn clone_shares_analog_and_la_storage() {
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let mut frame = crate::board::Frame { ch: [vec![1.0; 7], vec![2.0; 3], vec![], vec![]], ..Default::default() };
    frame.la = Some(crate::ui::la::LaRec { t0: 0.0, dt: 1.0, words: std::sync::Arc::new(vec![3; 123]) });
    let frame = std::sync::Arc::new(frame);
    let (copied, sample) = clone_frame(&frame);
    assert!(counters().text().contains("producer.clone n=0 total=0"));
    sample.record();
    assert_eq!(copied.ch, frame.ch);
    assert!(std::sync::Arc::ptr_eq(&copied, &frame));
    assert!(std::sync::Arc::ptr_eq(&copied.la.as_ref().unwrap().words, &frame.la.as_ref().unwrap().words));
    let text = counters().text();
    assert!(text.contains("producer.clone n=1 total=1"));
    assert!(!text.contains("producer.clone_size"));
}

#[test]
#[ignore = "explicit release micro-benchmark: OOS_PERF_TESTS=1, one test thread"]
fn bench_deep_diagnostics_overhead() {
    assert_eq!(std::env::var("OOS_PERF_TESTS").as_deref(), Ok("1"));
    if cfg!(debug_assertions) {
        panic!("overhead evidence requires release");
    }
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let shared = Mutex::new(0u64);
    let diagnostics = counters();
    let mut timing = super::super::StageTiming::default();
    let pair = crate::system::perfguard::measure(
        super::super::ROUNDS,
        1000,
        || {
            for _ in 0..12 {
                *lock(&shared) += 1;
            }
        },
        || {
            timing.intake_begin();
            for poll in &Poll::ALL[..16] {
                let wait = diagnostics.ui_wait();
                let started = Instant::now();
                std::hint::black_box(poll);
                let elapsed = started.elapsed();
                timing.poll(elapsed, diagnostics.ui_wait().wrapping_sub(wait));
                diagnostics.poll(*poll, elapsed);
            }
            for side in 0..12 {
                *timed_lock(&shared, diagnostics, side % 2) += 1;
            }
            timing.intake_end();
            timing.add(Duration::from_millis(1), Duration::ZERO);
        },
    );
    let best = pair.best.1.saturating_sub(pair.best.0);
    let tail = pair.p99.1.saturating_sub(pair.p99.0);
    println!("diagnostic overhead 16 polls + 12 locks: best={best:?} p99_cpu={tail:?}; limits 5/10 us");
    assert!(best < Duration::from_micros(5) && tail < Duration::from_micros(10));
    println!("{}", diagnostics.text());
}

fn median(text: &str, label: &str) -> u64 {
    text.split_once(&format!("diag {label} n=1 total=1 p50_ns="))
        .unwrap()
        .1
        .split_whitespace()
        .next()
        .unwrap()
        .parse()
        .unwrap()
}

#[test]
fn blocked_mutex_records_wait_longer_than_held() {
    let diagnostics = Diagnostics::default();
    let shared = Mutex::new(());
    let start = Instant::now();
    let mut clock = [start, start + Duration::from_millis(75)].into_iter();
    let mut sample = 0;
    let mut guard = timed_lock_with(&shared, &diagnostics, 0, || {
        assert_eq!(shared.try_lock().is_ok(), sample == 0, "lock order at sample {sample}");
        sample += 1;
        clock.next().unwrap()
    });
    assert_eq!(sample, 2);
    guard.record_at(start + Duration::from_millis(76));
    drop(guard);
    let text = diagnostics.text();
    assert_eq!(median(&text, "lock.ui.wait"), 75_000_000);
    assert_eq!(median(&text, "lock.ui.held"), 1_000_000);
}

#[test]
fn uncontended_mutex_records_held_longer_than_wait() {
    let diagnostics = Diagnostics::default();
    let shared = Mutex::new(());
    let start = Instant::now();
    let mut guard = timed_lock_with(&shared, &diagnostics, 0, || start);
    guard.record_at(start + Duration::from_millis(50));
    drop(guard);
    let text = diagnostics.text();
    assert_eq!(median(&text, "lock.ui.wait"), 0);
    assert_eq!(median(&text, "lock.ui.held"), 50_000_000);
}

#[test]
fn every_poll_label_has_its_own_slot() {
    let labels = [
        "memory",
        "relayout",
        "meas_hold",
        "workspace_hold",
        "update",
        "wlan",
        "ntp",
        "ssh",
        "usb",
        "ch_hold",
        "counter",
        "dvm",
        "mask",
        "zone",
        "la_probe",
        "log",
        "external",
    ];
    for (index, poll) in Poll::ALL.into_iter().enumerate() {
        let diagnostics = Diagnostics::default();
        diagnostics.poll(poll, Duration::from_nanos(index as u64 + 1));
        let text = diagnostics.text();
        assert_eq!(poll.label(), labels[index]);
        assert_eq!(median(&text, &format!("poll.{}", labels[index])), index as u64 + 1);
        for (other, label) in labels.iter().enumerate() {
            if other != index {
                assert!(text.contains(&format!("diag poll.{label} n=0 total=0")), "{text}");
            }
        }
    }
}
