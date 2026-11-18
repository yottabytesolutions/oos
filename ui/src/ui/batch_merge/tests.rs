// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::model::Model;
use crate::sparse::Interp;
use std::sync::mpsc;

fn pipe() -> &'static Pipe {
    let pipe = Box::leak(Box::new(Pipe::start(0, 0, Box::leak(Box::default()))));
    pipe.set_view(Some(&Model::default()), 4, 4, Interp::Exact);
    pipe
}

fn input() -> Input {
    Input { elapsed: 0.0, persist: 0.0, reset: false, wait: false, little: 0 }
}

fn completed(job: Job) -> Finished {
    let mut ph = job.ph;
    for map in &mut ph.map {
        map.resize(ph.w * ph.h, 0);
    }
    ph.map[0].fill(42);
    Finished {
        ph,
        cleared: true,
        key: job.key,
        merged: Merged { frames: 7, triggered: 5, ..Default::default() },
        edges: [1, 2, 3, 4],
        off: (3.0, 2.0),
        duration: std::time::Duration::ZERO,
    }
}

#[test]
fn pending_merge_keeps_the_visible_picture_and_publishes_once() {
    let pipe = pipe();
    let (worker, requests, results) = Worker::manual();
    let mut merge = Merge { worker: Some(worker), ..Default::default() };
    let mut ph = Phosphor::new(4, 4);
    ph.map[0].fill(3);
    let (mut clear, mut edges, mut off) = (true, [0; 4], (0.0, 0.0));
    merge.start(pipe, &ph, clear, 0);
    let job = requests.recv().unwrap();
    assert!(job.ph.map.iter().all(Vec::is_empty), "replacement jobs do not copy the visible picture");
    assert_eq!(merge.merge(pipe, &mut ph, &mut clear, &mut edges, &mut off, input()), Merged::default());
    assert_eq!(ph.map[0], [3; 16]);
    assert!(merge.busy());
    assert_eq!(requests.try_recv().err(), Some(mpsc::TryRecvError::Empty));
    results.send(completed(job)).unwrap();
    let got = merge.merge(pipe, &mut ph, &mut clear, &mut edges, &mut off, input());
    assert_eq!((got.frames, got.triggered), (7, 5));
    assert_eq!(ph.map[0], [42; 16]);
    assert_eq!(edges, [1, 2, 3, 4]);
    assert_eq!(off, (3.0, 2.0));
    assert!(!clear);
    assert!(!merge.busy());
    assert_eq!(merge.merge(pipe, &mut ph, &mut clear, &mut edges, &mut off, input()), Merged::default());
}

#[test]
fn obsolete_completions_cannot_restore_a_cleared_or_remapped_picture() {
    for reason in ["clear", "mapping", "inactive", "size"] {
        let pipe = pipe();
        let (worker, requests, results) = Worker::manual();
        let mut merge = Merge { worker: Some(worker), ..Default::default() };
        let mut ph = Phosphor::new(4, 4);
        merge.start(pipe, &ph, true, 0);
        let job = requests.recv().unwrap();
        let mut input = input();
        match reason {
            "clear" => input.reset = true,
            "mapping" => {
                let mut m = Model::default();
                m.ch[0].offset = 1.0;
                pipe.set_view(Some(&m), 4, 4, Interp::Exact);
            }
            "inactive" => merge.invalidate(),
            "size" => ph = Phosphor::new(3, 3),
            _ => unreachable!(),
        }
        results.send(completed(job)).unwrap();
        assert_eq!(merge.merge(pipe, &mut ph, &mut true, &mut [0; 4], &mut (0.0, 0.0), input), Merged::default());
        assert!(ph.map.iter().flatten().all(|&v| v == 0), "{reason}");
        assert!(!merge.busy());
    }
}

#[test]
fn delayed_persistence_uses_injected_picture_time() {
    let pipe = pipe();
    let (worker, requests, results) = Worker::manual();
    let mut merge = Merge { worker: Some(worker), ..Default::default() };
    let mut ph = Phosphor::new(4, 4);
    merge.start(pipe, &ph, false, 0);
    results.send(completed(requests.recv().unwrap())).unwrap();
    let input = Input { elapsed: 0.5, persist: 0.5, ..input() };
    merge.merge(pipe, &mut ph, &mut false, &mut [0; 4], &mut (0.0, 0.0), input);
    assert_eq!(ph.map[0], [15; 16]);
}

#[test]
#[ignore = "host Batch merge evidence: release, OOS_PERF_TESTS=1, one test thread"]
fn perf_guard_batch_picture_merge() {
    use crate::board::scripted::{self, Batch};
    use std::time::Instant;
    assert_eq!(std::env::var("OOS_PERF_TESTS").as_deref(), Ok("1"));
    for (tdiv, frames, parallel) in [(1e-6, 90, false), (10e-6, 14, false), (1e-6, 90, true), (10e-6, 14, true)] {
        let mut model = Model::default();
        model.tscale = crate::model::tscales().iter().position(|&v| (v - tdiv).abs() < tdiv * 1e-4).unwrap();
        model.acq = crate::model::AcqMode::Normal;
        model.dots = crate::model::DotsMode::Off;
        model.meas.clear();
        for channel in &mut model.ch {
            channel.on = true;
        }
        let pipe = Box::leak(Box::new(Pipe::start(4, 0, Box::leak(Box::default()))));
        pipe.set_view(Some(&model), 970, 448, Interp::Exact);
        let (fixture, frames, _) = scripted::draw_route_batch(&model, &Default::default(), frames);
        let (fb, decode, codes) = (fixture.frame_bytes(), fixture.decode(), fixture.codes());
        let mut bytes: Box<dyn crate::drawpipe::RawBatch> = Box::new(fixture.into_bytes());
        let (mut merge, mut ph) = (picture_merger(parallel), Phosphor::new(970, 448));
        let (mut ui_times, mut merge_times) = (Vec::new(), Vec::new());
        for round in 0..crate::perfguard::ROUNDS + 8 {
            pipe.feed_codes(bytes, fb, frames, 0, decode.clone(), codes.clone());
            bytes = crate::drawpipe::host::finish_batch(pipe, round as u64).unwrap();
            let start = Instant::now();
            merge.start(pipe, &ph, true, 0);
            let submit = start.elapsed();
            merge.worker.as_ref().unwrap().wait_ready("bounded Batch picture merge");
            let start = Instant::now();
            let got = merge.merge(pipe, &mut ph, &mut true, &mut [0; 4], &mut (0.0, 0.0), input());
            let ui = submit + start.elapsed();
            assert_eq!(got.frames as usize, frames);
            if round >= 8 {
                ui_times.push(ui.as_nanos());
                merge_times.push(merge.duration.as_nanos());
            }
        }
        ui_times.sort_unstable();
        merge_times.sort_unstable();
        println!("BATCH_PICTURE parallel={parallel} channels=4 acq=Normal tdiv={tdiv} frames={frames} merge_ns_p50={} merge_ns_p90={} merge_ns_p99={} ui_ns_p50={} ui_ns_p99={}",
            merge_times[50], merge_times[90], merge_times[99], ui_times[50], ui_times[99]);
        assert!(ph.map.iter().all(|map| map.iter().any(|&v| v > 0)));
        assert!(ui_times[99] < 33_000_000, "UI handoff must fit the picture period");
        assert!(merge_times[99] < 16_500_000, "merge must fit within half a picture period on the host");
        crate::drawpipe::shutdown::close(pipe, Instant::now() + super::super::lifecycle::test_wait::HANG_GUARD);
    }
}

#[test]
fn statistics_reset_keeps_the_picture_and_rejects_late_folds() {
    use oos_core::change::{Request, Shadow, UiAcq};
    let pipe = pipe();
    pipe.set_identity(Identity::default());
    let (worker, requests, results) = Worker::manual();
    let mut merge = Merge { worker: Some(worker), ..Default::default() };
    let mut ph = Phosphor::new(4, 4);
    merge.start(pipe, &ph, true, 0);
    let mut done = completed(requests.recv().unwrap());
    let mut fold = crate::framestats::Fold::default();
    fold.push(123.0);
    done.merged.folds.push(((oos_dsp::measure::Meas::Vavg, 0), fold));
    let model = Model::default();
    let config = crate::model::config();
    let mut shadow = Shadow::new(&model, UiAcq::default(), &config).unwrap();
    pipe.set_identity(shadow.observe(&model, UiAcq::default(), &[Request::StatsReset], &config).unwrap().identity());
    results.send(done).unwrap();
    let got = merge.merge(pipe, &mut ph, &mut true, &mut [0; 4], &mut (0.0, 0.0), input());
    assert_eq!(got.frames, 7);
    assert!(got.folds.is_empty());
    assert_eq!(ph.map[0], [42; 16]);
}

#[test]
fn live_steps_publish_successive_finished_pictures_without_an_empty_step_between_jobs() {
    use super::super::{change, host, Ui};
    use crate::board::scripted::{self, Batch, BatchKind, BatchSource};
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let mut ui = Ui::new(crate::wave::Source::Hw { shared: Default::default(), seen: 0 }, host::Host::fixture());
    ui.m.draw_batch = true;
    ui.m.meas.clear();
    ui.m.meas2.clear();
    ui.relayout();
    change::acquisition::script_prepare(&mut ui);
    let switches = change::acquisition::draw_switches(&ui, &ui.m);
    let pipe = crate::drawpipe::pipe(&switches).unwrap();
    let mut source = scripted::source(BatchKind::Raw, 123);
    let feed = |ui: &Ui, source: &mut scripted::ScriptedBatchSource, picture| {
        let id = change::acquisition::identity(ui);
        let batch = source.next_batch(&ui.m, &switches, 3, None).unwrap();
        let (fb, decode) = (batch.frame_bytes(), batch.decode());
        pipe.feed_with(Box::new(batch.into_bytes()), fb, 0..3, id, decode, None, Box::new(|| {}));
        crate::drawpipe::host::finish_batch(pipe, picture).unwrap();
    };
    feed(&ui, &mut source, 1);
    ui.step(1.0 / 30.0);
    assert_eq!(ui.wfm_total, 0);
    for picture in 1..=3 {
        ui.host.runtime().batch_merge.worker.as_ref().unwrap().wait_ready("next live Batch picture");
        if picture < 3 {
            feed(&ui, &mut source, picture + 1);
        }
        ui.step(1.0 / 30.0);
        assert_eq!(ui.wfm_total, picture * 3);
        assert!(ui.ph.map[0].iter().any(|&v| v > 0));
        assert_eq!(ui.host.runtime().batch_merge.busy(), picture < 3, "schedule only available producer maps");
    }
}

#[test]
#[ignore = "host Batch UI-step evidence: release, OOS_PERF_TESTS=1, one test thread"]
fn perf_guard_batch_ui_picture() {
    use super::super::{change, host, Ui};
    use crate::board::scripted::{self, Batch};
    use crate::wave::Source;
    use std::{sync::Arc, time::Instant};
    assert_eq!(std::env::var("OOS_PERF_TESTS").as_deref(), Ok("1"));
    for (tdiv, frames) in [(1e-6, 90), (10e-6, 14)] {
        let _scope = crate::drawpipe::test_scope::Scope::new();
        let mut ui = Ui::new(Source::Hw { shared: Default::default(), seen: 0 }, host::Host::fixture());
        let identity = prepare_live(&mut ui, tdiv);
        let switches = change::acquisition::draw_switches(&ui, &ui.m);
        let pipe = crate::drawpipe::pipe(&switches).unwrap();
        let (fixture, frames, _) = scripted::draw_route_batch(&ui.m, &switches, frames);
        let (fb, decode, codes) = (fixture.frame_bytes(), fixture.decode(), fixture.codes());
        let mut newest = fixture.newest(identity.epoch());
        newest.cfg = identity.gen();
        let mut bytes: Box<dyn crate::drawpipe::RawBatch> = Box::new(fixture.into_bytes());
        let mut times = Vec::new();
        for round in 0..crate::perfguard::ROUNDS + 9 {
            if let Some(worker) = &ui.host.runtime().batch_merge.worker {
                worker.wait_ready("completed live picture before timing UI intake");
            }
            let Source::Hw { shared, .. } = &ui.src else { unreachable!() };
            let shared = shared.clone();
            newest.seq += 1;
            let frame = Arc::new(newest.clone());
            pipe.feed_with(
                bytes,
                fb,
                0..frames - 1,
                identity,
                decode.clone(),
                codes.clone(),
                Box::new(move || {
                    let mut shared = oos_hal::sync::lock(&shared);
                    shared.frame = frame.clone();
                    shared.queue.push_back(frame);
                }),
            );
            bytes = crate::drawpipe::host::finish_batch(pipe, round as u64).unwrap();
            let started = Instant::now();
            ui.step(1.0 / 30.0);
            let elapsed = started.elapsed().as_nanos();
            assert_eq!(ui.wfm_total, (round * frames) as u64);
            if round >= 9 {
                times.push(elapsed);
            }
        }
        times.sort_unstable();
        println!(
            "BATCH_UI_PICTURE channels=4 acq=Normal tdiv={tdiv} frames={frames} ui_step_ns_p50={} ui_step_ns_p99={}",
            times[50], times[99]
        );
        assert!(times[99] < 33_000_000, "live UI step must fit the picture period");
    }
}

fn prepare_live(ui: &mut super::super::Ui, tdiv: f32) -> Identity {
    ui.m.draw_batch = true;
    ui.m.acq = crate::model::AcqMode::Normal;
    ui.m.tscale = crate::model::tscales().iter().position(|&v| (v - tdiv).abs() < tdiv * 1e-4).unwrap();
    ui.m.meas.clear();
    ui.m.meas2.clear();
    ui.m.dots = crate::model::DotsMode::Off;
    for channel in &mut ui.m.ch {
        channel.on = true;
    }
    ui.relayout();
    super::super::change::acquisition::script_prepare(ui)
}

#[test]
fn empty_completion_keeps_newer_visible_pixels_and_clear_intent() {
    let pipe = pipe();
    let (worker, requests, results) = Worker::manual();
    let mut merge = Merge { worker: Some(worker), ..Default::default() };
    let mut ph = Phosphor::new(4, 4);
    merge.start(pipe, &ph, true, 0);
    let mut done = completed(requests.recv().unwrap());
    done.merged = Merged::default();
    results.send(done).unwrap();
    ph.map[0].fill(17);
    let mut clear = true;
    assert_eq!(merge.merge(pipe, &mut ph, &mut clear, &mut [0; 4], &mut (0.0, 0.0), input()), Merged::default());
    assert_eq!(ph.map[0], [17; 16], "an empty completion cannot restore an older snapshot");
    assert!(clear);
    assert!(!merge.busy());
}

fn picture_merger(parallel: bool) -> Merge {
    if parallel {
        Merge::default()
    } else {
        let mut reduction = crate::drawpipe::reduction::Reduction::default();
        Merge {
            worker: Some(Worker::spawn("picture-serial", move |job| run(job, &mut reduction)).unwrap()),
            ..Default::default()
        }
    }
}
