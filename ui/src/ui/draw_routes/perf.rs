// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::board::Frame;
use crate::drawpipe::{CodesInto, DecodeInto};
use std::time::Instant;

mod coverage;

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
    pub(super) fn new(records: bool, channels: usize, tdiv: f32, frames: usize) -> Self {
        let mut route = Route::new(BatchKind::Codes, channels, tdiv);
        route.records = records;
        let (input, frames, plan) = scripted::draw_route_batch(&route.ui.m, route.ui.switches(), frames);
        println!("ROUTE_PLAN records={records} channels={channels} tdiv={tdiv} frames={frames} {plan}");
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

#[derive(Clone, Copy)]
enum Expectation {
    Exact,
    Coverage,
}

struct Case {
    channels: usize,
    tdiv: f32,
    requested: usize,
    frames: usize,
    weight_per_record: u64,
    expectation: Expectation,
}

impl Case {
    const fn new(
        channels: usize,
        tdiv: f32,
        (requested, frames): (usize, usize),
        weight_per_record: u64,
        expectation: Expectation,
    ) -> Self {
        Self { channels, tdiv, requested, frames, weight_per_record, expectation }
    }

    fn build(&self, records: bool, label: &str) -> Build {
        let build = Build::new(records, self.channels, self.tdiv, self.requested);
        assert_eq!(build.frames, self.frames, "{label}: planned frames (records={records})");
        build
    }
}

const CASES: [Case; 21] = {
    use Expectation::{Coverage, Exact};
    [
        Case::new(1, 200e-9, (1, 1), 27_456, Exact),
        Case::new(1, 200e-9, (48, 48), 27_456, Exact),
        Case::new(1, 10e-6, (1, 1), 79_296, Exact),
        Case::new(1, 10e-6, (48, 48), 79_296, Exact),
        Case::new(1, 10e-6, (0, 20), 79_296, Exact),
        Case::new(1, 200e-6, (1, 1), 1_173_696, Exact),
        Case::new(1, 200e-6, (48, 48), 1_173_696, Exact),
        Case::new(2, 200e-9, (1, 1), 30_048, Coverage),
        Case::new(2, 200e-9, (48, 48), 30_048, Coverage),
        Case::new(2, 10e-6, (1, 1), 79_296, Exact),
        Case::new(2, 10e-6, (48, 48), 79_296, Exact),
        Case::new(2, 10e-6, (0, 14), 79_296, Exact),
        Case::new(2, 200e-6, (1, 1), 1_173_696, Exact),
        Case::new(2, 200e-6, (48, 48), 1_173_696, Exact),
        Case::new(4, 200e-9, (1, 1), 30_624, Coverage),
        Case::new(4, 200e-9, (48, 48), 30_624, Coverage),
        Case::new(4, 10e-6, (1, 1), 79_296, Exact),
        Case::new(4, 10e-6, (48, 48), 79_296, Exact),
        Case::new(4, 10e-6, (0, 14), 79_296, Exact),
        Case::new(4, 200e-6, (1, 1), 1_173_696, Exact),
        Case::new(4, 200e-6, (48, 48), 1_173_696, Exact),
    ]
};

#[test]
#[ignore = "host route evidence: release, OOS_PERF_TESTS=1, one test thread"]
fn perf_guard_draw_routes() {
    use drawpipe::route::Route::{Dense, Upsampled};

    assert_eq!(std::env::var("OOS_PERF_TESTS").as_deref(), Ok("1"));
    for case in CASES {
        let Case { channels, tdiv, requested, frames, weight_per_record, expectation } = case;
        let label = format!("channels={channels} tdiv={tdiv} requested={requested}");
        let (records, size) = {
            let _scope = Scope::new();
            let mut build = case.build(true, &label);
            build.run();
            let ui = &build.route.ui;
            assert!(!ui.ph.is_wide(), "{label}: Records counts must be narrow");
            (picture(ui), (ui.ph.w, ui.ph.h))
        };

        let _scope = Scope::new();
        let mut build = case.build(false, &label);
        let steps = sample_batch_route(&mut build, channels, tdiv);
        let ui = &build.route.ui;
        assert!(!ui.ph.is_wide(), "{label}: Batch counts must be narrow");
        assert_eq!((ui.ph.w, ui.ph.h), size, "{label}: plot size");
        let batch = picture(ui);
        let route = match expectation {
            Expectation::Exact => {
                compare(&records, &batch, &label);
                Dense
            }
            Expectation::Coverage => {
                coverage::compare(&records, &batch, size.0, channels, &label);
                Upsampled
            }
        };

        // At minimum persistence each map contains only the current batch of identical records.
        let want = std::array::from_fn(|c| if c < channels { weight_per_record * frames as u64 } else { 0 });
        assert_eq!(batch.frames as usize, frames, "{label}: picture frames");
        assert_eq!(weights(&batch), want, "{label}: total deposited weight");
        assert_eq!(
            drawpipe::counters().routes.count(route),
            (steps * frames) as u64,
            "{label}: records through {route:?}"
        );
    }
}

fn sample_batch_route(build: &mut Build, channels: usize, tdiv: f32) -> usize {
    const WARMUP: usize = 8;
    let frames = build.frames;
    for _ in 0..WARMUP {
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
    println!(
        "DRAW_ROUTE Batch channels={channels} tdiv={tdiv} frames={frames} build_ns_best={} build_ns_p50={} build_ns_p99={} allocations_min={} allocations_max={}",
        times[0], times[50], times[99], allocations[0], allocations[100]
    );
    if channels == 4 && tdiv == 10e-6 && frames == 48 {
        println!("DRAW_ROUTE_PHASES records=false {}", change::timing(&build.route.ui).diagnostics_text());
        println!("DRAW_ROUTE_COST records=false before={cost_before} after={}", crate::claim_timing::summary());
    }
    WARMUP + crate::perfguard::ROUNDS
}

#[test]
#[ignore = "host draw profile: release, OOS_PERF_TESTS=1, one test thread"]
fn perf_guard_draw_route_deposit_profile() {
    assert_eq!(std::env::var("OOS_PERF_TESTS").as_deref(), Ok("1"));
    let _scope = Scope::new();
    let mut route = Route::new(BatchKind::Codes, 4, 10e-6);
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
            route.ui.draw_queued_prefix(frames.clone());
        },
    );
    assert!(pixels.maps() == route.ui.ph.maps(), "the per-frame pixel kernel and step span accumulation agree");
    println!("DRAW_DEPOSIT_PROFILE {plan} frames=48 pixel_per_frame_vs_step_difference_spans={pair:?}");
}

#[test]
fn draw_routes_one_frame_and_full_steps_match_with_buffer_reuse() {
    for channels in [1, 2, 4] {
        for frames in [1, 48] {
            let run = |records_route| {
                let _scope = Scope::new();
                let mut build = Build::new(records_route, channels, 10e-6, frames);
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
fn state_reports_the_effective_draw_route() {
    let _scope = Scope::new();
    let mut route = Route::new(BatchKind::Codes, 4, 1e-6);
    change::settle(&mut route.ui);
    let text = crate::cmd::stat_text(&route.ui);
    assert!(text.contains("route=batch"), "{text}");
    assert!(!text.contains(" pipeline "), "{text}");
    route.ui.m.zoom = true;
    assert!(crate::cmd::stat_text(&route.ui).contains("route=batch"));
    route.ui.m.z_src = Some(1);
    assert!(crate::cmd::stat_text(&route.ui).contains("route=records"));
    route.ui.m.run = RunState::Stop;
    assert!(crate::cmd::stat_text(&route.ui).contains("route=stopped"));
}

#[test]
fn picture_record_ring_samples_live_and_stopped_empty_steps() {
    let _scope = Scope::new();
    let mut route = Route::new(BatchKind::Codes, 1, 10e-6);
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
    for records_route in [true, false] {
        let _scope = Scope::new();
        let mut route = Route::new(BatchKind::Codes, 4, 10e-6);
        route.records = records_route;
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
        for records_route in [true, false] {
            let _scope = Scope::new();
            let mut build = Build::new(records_route, 4, tdiv, 48);
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
                "BATCH_PICTURE channels=4 tdiv={tdiv} records={records_route} frames=48 best_ns={} p50_ns={} p90_ns={} p99_ns={}",
                times[0], times[50], times[90], times[99]
            );
            println!(
                "BATCH_PICTURE_PHASES records={records_route} tdiv={tdiv} {}",
                change::timing(&build.route.ui).diagnostics_text()
            );
            pictures.push(picture(&build.route.ui));
        }
        compare(&pictures[0], &pictures[1], "F2 four-channel production-plan picture");
    }
}

#[test]
fn deposit_waveform_tap_counts_cheap_statistics_once() {
    let run = |records_route| {
        let _scope = Scope::new();
        let mut build = Build::new(records_route, 2, 10e-6, 48);
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
    let (records, deposit) = (run(true), run(false));
    assert!(!records.is_empty());
    for (a, b) in records.iter().zip(&deposit) {
        assert_eq!(a.n, b.n, "the pool and waveform reader must count each record once");
        assert_eq!(b.n, 144);
        assert!(a.sum.is_finite() && b.sum.is_finite());
    }
}

mod worker_short;

mod two_channel;
