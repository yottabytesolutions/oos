// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::system::test_allocator::count_own;
use crate::ui::tests::allocation_budget::{PEAK_ALLOCATIONS, WARMUP_STEPS};
use oos_hal::sync::lock;
use std::sync::atomic::Ordering::Relaxed;

#[test]
fn peak_burst_reaches_records_picture_measurements_and_history_across_steps() {
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let (mut ui, shared) = fixture(host::Host::fixture());
    let bound = crate::board::scripted::peak_batch_frames(&ui.m) as u64;
    let mut published = 0;
    let mut steps = 0;
    while ui.records < 720 {
        if lock(&shared).queue.is_empty() {
            let count = bound.min(720 - published);
            feed(&ui, &shared, published + 1, count);
            published += count;
        }
        let before = ui.records;
        ui.step(0.04);
        let got = ui.records - before;
        assert!((1..=crate::board::RECORDS_PER_STEP as u64).contains(&got), "the existing step bound must hold");
        assert_eq!(ui.last[0].as_ref().unwrap().raw.as_ref().unwrap().frame.seq, ui.records);
        assert!(ui.ph.maps().iter().flatten().any(|&v| v != 0));
        assert!(ui.stats[0].vavg.is_finite());
        steps += 1;
        assert!(steps <= 720);
    }
    assert!(steps >= 15, "a burst must span UI steps");
    assert_eq!(ui.records, 720);
    assert_eq!(ui.history.len(), HISTORY, "history keeps its existing bounded retention");
    assert!(ui.mstat[0].n > 0, "the existing measurement feed receives the retained records");
    let counters = crate::drawpipe::counters();
    assert_eq!(counters.losses.overflow.load(Relaxed), 0);
    assert_eq!(counters.dropped.load(Relaxed), 0);
    ui.step(0.04);
    assert_eq!(ui.records, 720, "an empty step cannot duplicate a capture");
}

fn fixture(host: host::Host) -> (Ui, crate::board::HwShared) {
    let shared = crate::board::HwShared::default();
    let mut ui = Ui::new(Source::Hw { shared: shared.clone(), seen: 0 }, host);
    ui.m.acq = AcqMode::Peak;
    ui.m.meas = vec![(Meas::Vavg, WaveId::Ch(0))];
    ui.m.meas2.clear();
    ui.m.persist = PERSIST.len() - 1;
    ui.step(0.04);
    (ui, shared)
}

fn feed(ui: &Ui, shared: &crate::board::HwShared, first: u64, count: u64) {
    let frames = (first..first + count)
        .map(|seq| {
            let mut frame = super::frame(ui, seq);
            // Keep an early transient distinct from every later capture.
            frame.ch[0].fill(if seq == 1 { -0.8 } else { (seq % 11) as f32 / 20.0 });
            frame.basic[0] = crate::board::BasicStats::of(&frame.ch[0]);
            frame
        })
        .collect();
    assert!(crate::board::scripted::peak_batch(shared, frames).is_ok());
}

#[test]
fn peak_burst_matches_lossless_small_batch_picture_statistics_and_history() {
    let _scope = crate::drawpipe::test_scope::Scope::new();
    // Deterministic steps compare the entire result with small batches that never
    // overflowed before the fix. The separate live-budget test exercises yielding.
    let (mut burst, bs) = fixture(host::Host::headless());
    let (mut reference, rs) = fixture(host::Host::headless());
    burst.m.mdepth = 1;
    reference.m.mdepth = 1;
    burst.step(0.04);
    reference.step(0.04);
    let mut published = 0;
    let bound = crate::board::scripted::peak_batch_frames(&burst.m) as u64;
    for batch in 0..15 {
        if lock(&bs).queue.is_empty() {
            let count = bound.min(720 - published);
            feed(&burst, &bs, published + 1, count);
            published += count;
        }
        feed(&reference, &rs, batch * 48 + 1, 48);
        reference.step(0.04);
        burst.step(0.04);
    }
    assert_eq!((burst.records, reference.records), (720, 720));
    assert_eq!(burst.ph.maps(), reference.ph.maps(), "the early transient must contribute to persistence");
    assert_eq!((burst.mstat[0].n, reference.mstat[0].n), (15, 15), "preserve the current per-step measurement cadence");
    assert_eq!(burst.mstat[0].sum, reference.mstat[0].sum);
    assert_eq!(burst.mstat[0].min, reference.mstat[0].min);
    assert_eq!(burst.mstat[0].max, reference.mstat[0].max);
    assert_eq!(burst.history.len(), HISTORY);
    for (actual, expected) in burst.history.iter().zip(&reference.history) {
        let bits = |r: &Record| r.ch[0].iter().map(|x| x.to_bits()).collect::<Vec<_>>();
        assert_eq!(bits(actual), bits(expected));
    }
}

#[test]
#[ignore = "explicit allocation guard; run with OOS_PERF_TESTS=1"]
fn perf_guard_peak_intake_change_budget() {
    if std::env::var("OOS_PERF_TESTS").as_deref() != Ok("1") {
        return;
    }
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let (mut ui, shared) = fixture(host::Host::fixture());
    for step in 0..300 {
        if lock(&shared).queue.is_empty() {
            feed(&ui, &shared, ui.records + 1, crate::board::scripted::peak_batch_frames(&ui.m) as u64);
        }
        let (allocations, _) = count_own(|| change::begin_step(&mut ui, 1.0 / 30.0));
        if step >= WARMUP_STEPS {
            assert!(
                allocations <= PEAK_ALLOCATIONS,
                "Peak backlog intake/change allocations {allocations} at step {step}"
            );
        }
        ui.step(1.0 / 30.0);
    }
}

#[test]
#[ignore = "explicit release step-count guard; run alone with OOS_PERF_TESTS=1"]
fn perf_guard_peak_publication_to_display_steps() {
    if std::env::var("OOS_PERF_TESTS").as_deref() != Ok("1") {
        return;
    }
    let _scope = crate::drawpipe::test_scope::Scope::new();
    for host in [host::Host::headless(), host::Host::fixture()] {
        let (mut ui, shared) = fixture(host);
        ui.m.mdepth = 1;
        ui.step(0.04);
        let count = crate::board::scripted::peak_batch_frames(&ui.m) as u64;
        feed(&ui, &shared, 1, count);
        let mut steps = 0;
        while !lock(&shared).queue.is_empty() {
            ui.step(0.04);
            steps += 1;
            assert!(steps <= count);
        }
        let shown = ui.last[0].as_ref().unwrap().raw.as_ref().unwrap().frame.seq;
        assert_eq!((ui.records, shown), (count, count), "newest published capture reaches the display");
        println!("Peak publication-to-display: batch={count} steps={steps} time_budgets={}", ui.host.time_budgets());
        assert!(steps <= 2, "maximum Peak batch took {steps} steps from publication to display (budget 2)");
    }
}
