// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::board::Frame;
use crate::drawpipe::{CodesInto, DecodeInto};
use std::time::Instant;

pub(super) struct Build {
    pub(super) route: Route,
    input: Input,
    pub(super) frames: usize,
}

struct Input {
    bytes: Option<Box<dyn RawBatch>>,
    decode: DecodeInto,
    codes: Option<CodesInto>,
    fb: usize,
    frames: usize,
    first: u64,
}

impl Build {
    pub(super) fn new(batch: bool, channels: usize, tdiv: f32, frames: usize) -> Self {
        let route = Route::new(BatchKind::Codes, batch, channels, tdiv);
        let (input, frames, plan) = scripted::draw_route_batch(&route.ui.m, route.ui.switches(), frames);
        println!("ROUTE_PLAN batch={batch} channels={channels} tdiv={tdiv} frames={frames} {plan}");
        let (fb, decode, codes) = (input.frame_bytes(), input.decode(), input.codes());
        let input = Input { bytes: Some(Box::new(input.into_bytes())), decode, codes, fb, frames, first: 1 };
        Self { route, input, frames }
    }

    pub(super) fn run(&mut self) {
        self.input.first = self.route.ui.wfm_total + 1;
        self.input.bytes = Some(self.route.dispatch(self.frames, |_, _| &mut self.input));
    }
}

impl Batch for &mut Input {
    type Bytes = Box<dyn RawBatch>;

    fn frame_bytes(&self) -> usize {
        self.fb
    }

    fn newest(&self, stamp: u64) -> Frame {
        self.frame(self.frames - 1, stamp)
    }

    fn frame(&self, index: usize, stamp: u64) -> Frame {
        let mut frame = Frame::default();
        let bytes = self.bytes.as_ref().unwrap().bytes();
        (self.decode)(&bytes[index * self.fb..(index + 1) * self.fb], &mut frame);
        (frame.seq, frame.stamp) = (self.first + index as u64, stamp);
        frame
    }

    fn decode(&self) -> DecodeInto {
        self.decode.clone()
    }

    fn codes(&self) -> Option<CodesInto> {
        self.codes.clone()
    }

    fn into_bytes(self) -> Self::Bytes {
        self.bytes.take().unwrap()
    }
}

#[test]
#[ignore = "host route evidence: release, OOS_PERF_TESTS=1, one test thread"]
fn perf_guard_draw_routes() {
    assert_eq!(std::env::var("OOS_PERF_TESTS").as_deref(), Ok("1"));
    for channels in [1, 2, 4] {
        for tdiv in [200e-9, 10e-6, 200e-6] {
            for frames in [1, 48].into_iter().chain((tdiv == 10e-6).then_some(0)) {
                let mut pictures = Vec::new();
                // Each scope owns its pipe. Sampling is isolated from other test cases by --test-threads=1.
                for batch in [false, true] {
                    let _scope = Scope::new();
                    let mut build = Build::new(batch, channels, tdiv, frames);
                    let frames = build.frames;
                    for _ in 0..8 {
                        build.run();
                    }
                    let cost_before = crate::claim_timing::summary();
                    let mut times = Vec::new();
                    let mut allocations = Vec::new();
                    for _ in 0..crate::perfguard::ROUNDS {
                        let start = Instant::now();
                        let (count, ()) = HEAP.count(|| build.run());
                        times.push(start.elapsed().as_nanos());
                        allocations.push(count);
                    }
                    times.sort_unstable();
                    allocations.sort_unstable();
                    println!("DRAW_ROUTE {} channels={channels} tdiv={tdiv} frames={frames} build_ns_best={} build_ns_p50={} build_ns_p99={} allocations_min={} allocations_max={}",
                        if batch { "Batch" } else { "Standard" }, times[0], times[50], times[99], allocations[0], allocations[100]);
                    if channels == 4 && tdiv == 10e-6 && frames == 48 {
                        println!(
                            "DRAW_ROUTE_PHASES batch={batch} {}",
                            change::timing(&build.route.ui).diagnostics_text()
                        );
                        println!(
                            "DRAW_ROUTE_COST batch={batch} before={cost_before} after={}",
                            crate::claim_timing::summary()
                        );
                    }
                    assert_eq!(build.route.ui.batch_picture.frames as usize, frames);
                    pictures.push(picture(&build.route.ui));
                }
                compare(&pictures[0], &pictures[1], "warmed production-plan route build");
            }
        }
    }
}

#[test]
#[ignore = "host draw profile: release, OOS_PERF_TESTS=1, one test thread"]
fn perf_guard_draw_route_deposit_profile() {
    assert_eq!(std::env::var("OOS_PERF_TESTS").as_deref(), Ok("1"));
    let _scope = Scope::new();
    let mut route = Route::new(BatchKind::Codes, false, 4, 10e-6);
    let (input, _, plan) = scripted::draw_route_batch(&route.ui.m, route.ui.switches(), 48);
    let frames: Vec<_> = (0..48).map(|i| Arc::new(input.frame(i, 0))).collect();
    let mut pixels = Phosphor::new(route.ui.ph.w, route.ui.ph.h);
    let model = route.ui.m.clone();
    let rounds = if std::env::var_os("OOS_DRAW_PROFILE").is_some() { 1001 } else { crate::perfguard::ROUNDS };
    let pair = crate::system::perfguard::measure(
        rounds,
        1,
        || {
            pixels.clear();
            for f in &frames {
                drawpipe::draw_frame(&mut pixels, &model, f, crate::sparse::Interp::Exact, None);
            }
        },
        || {
            route.ui.ph.clear();
            route.ui.fast_draw_all(frames.clone());
        },
    );
    assert!(pixels.maps() == route.ui.ph.maps(), "the per-frame pixel kernel and step span accumulation agree");
    println!("DRAW_DEPOSIT_PROFILE {plan} frames=48 pixel_per_frame_vs_step_difference_spans={pair:?}");
}

#[test]
fn draw_routes_one_frame_and_full_steps_match_with_buffer_reuse() {
    for channels in [1, 2, 4] {
        for frames in [1, 48] {
            let run = |batch| {
                let _scope = Scope::new();
                let mut build = Build::new(batch, channels, 10e-6, frames);
                for step in 1..=2 {
                    build.run();
                    assert_eq!(build.route.ui.wfm_total, (step * frames) as u64);
                    let Source::Hw { shared, .. } = &build.route.ui.src else { unreachable!() };
                    assert_eq!(lock(shared).frame.seq, (step * frames) as u64);
                }
                picture(&build.route.ui)
            };
            compare(&run(false), &run(true), "one-frame/48-frame production sample geometry");
        }
    }
}

#[test]
fn state_reports_the_effective_pipeline_and_draw_route() {
    let _scope = Scope::new();
    let mut route = Route::new(BatchKind::Codes, true, 4, 1e-6);
    for (batch, pipeline, draw) in [(false, "Frames", "standard"), (true, "Batch", "batch")] {
        route.ui.m.draw_batch = batch;
        change::settle(&mut route.ui);
        let text = crate::cmd::stat_text(&route.ui);
        assert!(text.contains(&format!("pipeline {pipeline} route={draw}")), "{text}");
    }
    route.ui.m.zoom = true;
    assert!(crate::cmd::stat_text(&route.ui).contains("route=records"));
    route.ui.m.run = RunState::Stop;
    assert!(crate::cmd::stat_text(&route.ui).contains("route=stopped"));
}

#[test]
fn picture_record_ring_samples_live_and_stopped_empty_steps() {
    let _scope = Scope::new();
    let mut route = Route::new(BatchKind::Codes, true, 1, 10e-6);
    route.step(3);
    route.ui.step(1.0 / 30.0);
    let text = change::timing(&route.ui).diagnostics_text();
    assert!(text.contains("draw.frames_per_picture n=2 total=2 p50_frames=0 p90_frames=3"), "{text}");
    route.ui.m.run = RunState::Stop;
    route.ui.step(1.0 / 30.0);
    assert!(change::timing(&route.ui).diagnostics_text().contains("draw.frames_per_picture n=3 total=3"));
}

#[test]
fn quiescent_live_routes_conserve_read_deposited_and_shown_records() {
    for batch in [false, true] {
        let _scope = Scope::new();
        let mut route = Route::new(BatchKind::Codes, batch, 4, 10e-6);
        let counters = crate::drawpipe::counters();
        for records in [1, 27] {
            // The host fixture supplies completed reads without hardware IO.
            crate::drawpipe::Counters::add(&counters.acquired, records as u64);
            route.step(records);
        }
        assert_eq!(counters.read(), (28, 28, 0));
        let text = counters.records_text();
        assert!(text.contains("records read=28 deposited=28 shown=28"), "{text}");
    }
}

#[test]
#[ignore = "host F2 evidence: release, OOS_PERF_TESTS=1, one test thread"]
fn perf_guard_batch_picture_four_channels() {
    assert_eq!(std::env::var("OOS_PERF_TESTS").as_deref(), Ok("1"));
    for tdiv in [1e-6, 10e-6] {
        let mut pictures = Vec::new();
        for batch in [false, true] {
            let _scope = Scope::new();
            let mut build = Build::new(batch, 4, tdiv, 48);
            for _ in 0..8 {
                build.run();
            }
            let mut times = Vec::new();
            for _ in 0..crate::perfguard::ROUNDS {
                let started = Instant::now();
                build.run();
                times.push(started.elapsed().as_nanos());
            }
            times.sort_unstable();
            println!(
                "BATCH_PICTURE channels=4 tdiv={tdiv} batch={batch} frames=48 best_ns={} p50_ns={} p90_ns={} p99_ns={}",
                times[0], times[50], times[90], times[99]
            );
            println!(
                "BATCH_PICTURE_PHASES batch={batch} tdiv={tdiv} {}",
                change::timing(&build.route.ui).diagnostics_text()
            );
            pictures.push(picture(&build.route.ui));
        }
        compare(&pictures[0], &pictures[1], "F2 four-channel production-plan picture");
    }
}

#[test]
fn deposit_waveform_tap_counts_cheap_statistics_once() {
    let run = |batch| {
        let _scope = Scope::new();
        let mut build = Build::new(batch, 2, 10e-6, 48);
        build.route.ui.m.meas = crate::framestats::CHEAP.iter().map(|&kind| (kind, WaveId::Ch(0))).collect();
        build.run();
        build.route.ui.mstat.fill(MStat::default());
        for _ in 0..3 {
            build.run();
        }
        let counted = build.route.ui.measurement_statistics_snapshot_for_test();
        build.route.ui.step(1.0 / 30.0);
        assert_eq!(build.route.ui.measurement_statistics_snapshot_for_test(), counted);
        build.route.ui.mstat
    };
    let (standard, deposit) = (run(false), run(true));
    assert!(!standard.is_empty());
    for (a, b) in standard.iter().zip(&deposit) {
        assert_eq!(a.n, b.n, "the pool and waveform reader must count each record once");
        assert_eq!(b.n, 144);
        assert!(a.sum.is_finite() && b.sum.is_finite());
    }
}
