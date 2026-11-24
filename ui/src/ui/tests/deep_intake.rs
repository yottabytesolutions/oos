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
    // Construct the next acquisition outside the timed main step.
    let mut newest = frame.clone();
    newest.seq = seq;
    let newest = std::sync::Arc::new(newest);
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
    lock(&shallow_shared).frame = Default::default();
    lock(&deep_shared).frame = frame.into();
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
    lock(&shared).frame = frame.into();
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
                // Replay publication handle cloning under Shared.
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
        report("producer handle clone under lock", worker.join().unwrap());
    });
    println!("controlled clone overlap: {}", change::timing(&ui).text());
}

#[test]
fn deep_step_shares_acquired_samples_with_latest_records_and_measurements() {
    let _scope = crate::drawpipe::test_scope::Scope::new();
    for raw in [false, true] {
        let (mut ui, shared, mut frame) = fixture();
        if raw {
            frame.envelope = false;
            frame.dt /= 16.0;
            frame.ch = std::array::from_fn(|_| vec![1.0; crate::compress::MAX_DEPTH as usize + 1]);
            frame.pre = frame.ch[0].len() / 2;
            frame.acq.1 = crate::compress::MAX_DEPTH as usize + 1;
        }
        frame.seq = 2;
        let samples = frame.ch.each_ref().map(|v| v.as_ptr());
        let acquired = std::sync::Arc::new(frame);
        {
            let mut g = lock(&shared);
            g.frame = acquired.clone();
            g.queue.push_back(acquired.clone());
        }
        ui.step(1.0 / 30.0);
        let record = ui.last[0].as_ref().unwrap();
        let retained = &record.raw.as_ref().unwrap().frame;
        assert!(std::sync::Arc::ptr_eq(&acquired, retained));
        assert!(std::sync::Arc::ptr_eq(&acquired, &lock(&shared).frame));
        assert_eq!(samples, retained.ch.each_ref().map(|v| v.as_ptr()));
        for c in 0..NCH {
            let (measurement, i0, i1) = ui.meas_frame(record, c).unwrap();
            assert!(std::sync::Arc::ptr_eq(&acquired, &measurement));
            assert!(i1 > i0);
        }
        // Reading samples for autoset must leave the resident acquisition shared too.
        assert_eq!(ui.src.probe(0, 0.0, acquired.dt, 8).len(), 8);
        assert_eq!(samples, lock(&shared).frame.ch.each_ref().map(|v| v.as_ptr()));
    }
}

#[test]
fn queued_raw_before_pair_keeps_measurement_sample_identity() {
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let (mut ui, shared, mut pair) = fixture();
    ui.m.meas = vec![(Meas::Vavg, WaveId::Ch(0))];
    ui.step(1.0 / 30.0);
    let id = change::acquisition::identity(&ui);
    (pair.cfg, pair.stamp, pair.seq) = (id.gen(), id.epoch(), 3);
    let raw = std::sync::Arc::new(Frame {
        seq: 2,
        envelope: false,
        ch: std::array::from_fn(|_| vec![1.0; crate::compress::MAX_DEPTH as usize + 1]),
        ..pair.clone()
    });
    let pair = std::sync::Arc::new(pair);
    lock(&shared).read_geometry.deep_ok = true;
    let (_done, rx) = std::sync::mpsc::channel();
    let (tx, _jobs) = std::sync::mpsc::channel();
    let (_analysis, analysis_rx) = std::sync::mpsc::channel();
    ui.meas_bg = Some(MeasWorker {
        tx,
        rx: rx.into(),
        analysis_rx: analysis_rx.into(),
        analysis_busy: false,
        busy: true,
        accept: true,
        tap: None,
        join: std::thread::spawn(|| {}),
    });
    assert!(ui.fast_ok());
    assert!(crate::deepmeas::want(&ui.m).is_some());
    {
        let mut g = lock(&shared);
        g.frame = pair.clone();
        g.queue.extend([raw.clone(), pair.clone()]);
    }
    ui.step(1.0 / 30.0);
    assert!(lock(&shared).queue.is_empty());
    let rec = ui.meas_pending.as_ref().unwrap();
    assert!(std::sync::Arc::ptr_eq(&raw, &rec.raw.as_ref().unwrap().frame));
    assert!(std::sync::Arc::ptr_eq(&pair, &lock(&shared).frame));
    for c in 0..NCH {
        let (measured, lo, hi) = ui.meas_frame(rec, c).unwrap();
        assert!(std::sync::Arc::ptr_eq(&raw, &measured));
        assert!(hi > lo);
        // Math reads the displayed pair here; its raw input identity is pinned separately below.
        assert_eq!(measured.ch[c].as_ptr(), raw.ch[c].as_ptr());
    }
}

#[test]
fn math_ch_input_shares_the_entire_raw_capture_and_time_origin() {
    let (mut ui, _, mut frame) = fixture();
    frame.envelope = false;
    let frame = std::sync::Arc::new(frame);
    ui.last[0] = Some(crate::wave::hw_record(&ui.m, frame.clone()));
    let raw = ui.last[0].as_ref().unwrap().raw.as_ref().unwrap();
    for c in 0..NCH {
        let math = ui.math_source_record(c, 0).unwrap();
        assert_eq!(math.v.as_ptr(), frame.ch[c].as_ptr());
        assert_eq!(math.v.len(), frame.ch[c].len());
        assert_eq!(math.t0, raw.t0 - raw.i0 as f64 * frame.dt);
        assert_eq!(math.dt, frame.dt);
    }
    let held = ui.math_source_record(0, 0).unwrap();
    ui.last[0] = None;
    assert_eq!(held.v.as_ptr(), frame.ch[0].as_ptr());
    assert!(std::sync::Arc::strong_count(&frame) > 1);
}

#[test]
fn deep_pair_step_resamples_once_and_matches_the_double_conversion() {
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let (mut old, old_shared, frame) = fixture();
    let (mut new, new_shared, _) = fixture();
    feed(&old_shared, &frame, 2);
    feed(&new_shared, &frame, 2);
    // Prime the independent zoom-detail cache to isolate the two main-view conversions.
    for (ui, shared) in [(&old, &old_shared), (&new, &new_shared)] {
        let rec = crate::wave::hw_record(&ui.m, lock(shared).frame.clone());
        ui.captured_record_view(&rec, (tscales()[ui.m.zscale], ui.m.zdelay));
    }
    let before = crate::board::record::resample_count();
    crate::drawpipe::reuse::without_reuse(|| old.step(1.0 / 30.0));
    assert_eq!(crate::board::record::resample_count() - before, 2);
    let before = crate::board::record::resample_count();
    new.step(1.0 / 30.0);
    assert_eq!(crate::board::record::resample_count() - before, 1);
    assert_eq!(new.ph.maps(), old.ph.maps());
    assert_eq!(new.records, old.records);
    assert_eq!(new.m.status, old.m.status);
    for c in 0..NCH {
        let bits = |ui: &Ui| ui.last[0].as_ref().unwrap().ch[c].iter().map(|v| v.to_bits()).collect::<Vec<_>>();
        assert_eq!(bits(&new), bits(&old));
    }
}
