// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::board::Frame;
use crate::drawpipe::{CodesInto, DecodeInto};
use std::time::Instant;

struct Build {
    route: Route,
    input: Input,
    frames: usize,
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
    fn new(batch: bool, channels: usize, tdiv: f32, frames: usize) -> Self {
        let route = Route::new(BatchKind::Codes, batch, channels, tdiv);
        let (input, frames, plan) = scripted::draw_route_batch(&route.ui.m, route.ui.switches(), frames);
        println!("ROUTE_PLAN batch={batch} channels={channels} tdiv={tdiv} frames={frames} {plan}");
        let (fb, decode, codes) = (input.frame_bytes(), input.decode(), input.codes());
        let input = Input { bytes: Some(Box::new(input.into_bytes())), decode, codes, fb, frames, first: 1 };
        Self { route, input, frames }
    }

    fn run(&mut self) {
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
                    let cost_before = crate::framecost::summary();
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
                            crate::framecost::summary()
                        );
                    }
                    assert_eq!(build.route.ui.pic_frames as usize, frames);
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
    assert!(pixels.map == route.ui.ph.map, "the per-frame pixel kernel and step span accumulation agree");
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
