// SPDX-License-Identifier: GPL-2.0-or-later

//! Host diagnosis of deep intake. No acquisition device, GPU or hardware is started.

use super::super::*;
use crate::board::{Frame, HwShared};
use oos_hal::sync::lock;
use std::time::Instant;

fn fixture() -> (Ui, HwShared, Frame) {
    let shared = HwShared::default();
    let mut ui = Ui::new(Source::Hw { shared: shared.clone(), seen: 0 }, host::Host::headless());
    ui.m.run = RunState::Run;
    ui.m.meas.clear();
    ui.m.meas2.clear();
    ui.m.tscale = tscales().iter().position(|&t| (t - 1e-3).abs() < 1e-9).unwrap();
    ui.m.mdepth = board().depths().iter().position(|d| d.1 == 10_000_000).unwrap();
    for c in &mut ui.m.ch {
        c.on = true;
    }
    ui.step(1.0 / 30.0);
    let mut frame = crate::board::scripted::deep_intake_frame(&ui.m, ui.switches());
    let id = change::acquisition::identity(&ui);
    (frame.cfg, frame.stamp) = (id.gen(), id.epoch());
    (ui, shared, frame)
}

fn feed(shared: &HwShared, frame: &Frame, seq: u64) {
    // Model the producer's existing latest-frame copy; keep it outside the timed main step.
    let mut newest = frame.clone();
    newest.seq = seq;
    let mut g = lock(shared);
    g.frame = newest.clone();
    g.queue.push_back(newest);
}

#[test]
fn deep_pairs_cross_real_intake_and_record_conversion() {
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let (mut ui, shared, frame) = fixture();
    let before = ui.records;
    feed(&shared, &frame, 2);
    ui.step(1.0 / 30.0);
    assert_eq!(ui.records, before + 1);
    assert!(lock(&shared).queue.is_empty());
    let record = ui.last[0].as_ref().unwrap();
    let raw = record.raw.as_ref().unwrap();
    assert_eq!(raw.frame.ch.each_ref().map(Vec::len), [625_000; 4]);
    assert!(raw.frame.envelope);
    assert_eq!(record.ch.each_ref().map(Vec::len), [3600; 4]);
}

#[test]
#[ignore = "explicit release diagnosis; run alone with OOS_PERF_TESTS=1"]
fn bench_deep_intake() {
    assert_eq!(std::env::var("OOS_PERF_TESTS").as_deref(), Ok("1"));
    let (mut ui, shared, frame) = fixture();
    let mut record_steps = Vec::new();
    let mut idle_steps = Vec::new();
    let before = ui.records;
    for step in 0..300 {
        if step % 8 == 0 {
            feed(&shared, &frame, step + 2);
        }
        let start = Instant::now();
        ui.step(1.0 / 30.0);
        let elapsed = start.elapsed();
        if step % 8 == 0 {
            record_steps.push(elapsed);
        } else {
            idle_steps.push(elapsed);
        }
    }
    assert_eq!(ui.records - before, 38);
    assert!(lock(&shared).queue.is_empty());
    println!("deep cadence: {}", change::timing(&ui).text());
    report("record step", record_steps);
    report("idle step", idle_steps);
    // Alternate a shallow and deep resident frame through the actual intake/change seam.
    // No record conversion or drawing is included in these paired host distributions.
    let (mut shallow, shallow_shared, _) = fixture();
    let (mut deep, deep_shared, _) = fixture();
    lock(&shallow_shared).frame = Frame::default();
    lock(&deep_shared).frame = frame;
    let pair =
        crate::system::perfguard::measure(crate::perfguard::ROUNDS, 20, || seam(&mut shallow), || seam(&mut deep));
    println!("resident shallow/deep intake+change: best {:?}, p99 CPU {:?}", pair.best, pair.p99);
}

fn seam(ui: &mut Ui) {
    assert!(change::begin_step(ui, 1.0 / 30.0).is_some());
    change::finish_step(ui);
}

fn report(name: &str, mut times: Vec<std::time::Duration>) {
    times.sort_unstable();
    let sum: std::time::Duration = times.iter().sum();
    println!(
        "{name}: n={} mean={:?} p50={:?} p99={:?}",
        times.len(),
        sum / times.len() as u32,
        times[times.len() / 2],
        times[(99 * times.len()).div_ceil(100) - 1]
    );
}

#[test]
#[ignore = "controlled lock-contention diagnosis; run alone with OOS_PERF_TESTS=1"]
fn bench_deep_intake_lock() {
    assert_eq!(std::env::var("OOS_PERF_TESTS").as_deref(), Ok("1"));
    let (mut ui, shared, frame) = fixture();
    lock(&shared).frame = frame;
    let (ready, entered) = std::sync::mpsc::sync_channel(0);
    let (next, proceed) = std::sync::mpsc::sync_channel(0);
    std::thread::scope(|scope| {
        let worker = scope.spawn(move || {
            let mut holds = Vec::new();
            for _ in 0..64 {
                proceed.recv().unwrap();
                let mut g = lock(&shared);
                ready.send(()).unwrap();
                let start = Instant::now();
                // Replay the existing producer's sample copy under Shared, not new M2.3b work.
                g.frame = std::hint::black_box(g.frame.clone());
                holds.push(start.elapsed());
            }
            holds
        });
        for _ in 0..64 {
            next.send(()).unwrap();
            entered.recv().unwrap();
            seam(&mut ui);
        }
        report("producer sample copy under lock", worker.join().unwrap());
    });
    println!("controlled clone overlap: {}", change::timing(&ui).text());
}
