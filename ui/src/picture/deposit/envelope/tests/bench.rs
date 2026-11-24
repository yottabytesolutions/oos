// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use std::{hint::black_box, time::Instant};

#[test]
#[ignore = "host timing comparison, set OOS_PERF_TESTS=1 and run alone in release"]
fn measured_envelope_layouts() {
    assert_eq!(std::env::var("OOS_PERF_TESTS").as_deref(), Ok("1"));
    measure::<1>(0b0001, 14064);
    measure::<2>(0b1010, 7032);
    measure::<4>(0b1111, 3520);
    measure::<4>(0b1101, 3520);
}

fn measure<const N: usize>(mask: u32, len: usize) {
    let mut rng = Rng::new(515 + u64::from(mask));
    let words = words::<N>(&mut rng, len);
    for offsets in [None, Some(random_offsets(&mut rng, false))] {
        let record = Record::new(mask, words.clone(), offsets);
        for density in [3.1, 6.25, 30.0] {
            measure_density(&record, mask, density);
        }
    }
}

fn measure_density<const N: usize>(record: &Record<N>, mask: u32, density: f64) {
    let n = record.words.len();
    let width = (n as f64 / density).ceil() as usize + 1;
    let proj = record.projection(width, 480, density, 0.25);
    let codes: [Option<Codes<'_>>; 4] = std::array::from_fn(|c| record.codes(c));
    let maps: [Option<Map>; 4] = std::array::from_fn(|c| kernel::raw_map(&proj, c, 0.0));
    let first_map = maps.iter().flatten().next().unwrap();
    let mut edges = ColumnEdges::with_capacity(width);
    let mut out = Envelopes::with_capacity(width);
    let mut spans: [Spans; N] = std::array::from_fn(|_| Spans { c0: 0, rows: Vec::with_capacity(width) });
    let mut cache: [RowCache; N] = std::array::from_fn(|_| RowCache::default());
    let mut previous = [Spans::default(), Spans::default(), Spans::default(), Spans::default()];
    let samples = N * n;
    let old = ns_per_sample(samples, || {
        for c in 0..4 {
            if let Some((codes, map)) = codes[c].zip(maps[c]) {
                fastdraw::spans_into(black_box(proj.cols), proj.rows, &codes, &map, &mut previous[c]);
            }
        }
        black_box(&previous);
    });
    let column_edges = ns_per_sample(samples, || {
        edges.update(width, n, black_box(first_map));
        black_box(&edges);
    });
    edges.update(width, n, first_map);
    let kernel = ns_per_sample(samples, || {
        envelope(black_box(&record.words), &record.correction, black_box(&edges), &record.lanes, &mut out);
        black_box(&out);
    });
    let full = ns_per_sample(samples, || {
        edges.update(width, n, black_box(first_map));
        envelope(black_box(&record.words), &record.correction, &edges, &record.lanes, &mut out);
        for (lane, span) in spans.iter_mut().enumerate() {
            if let Some(c) = record.lanes.channel(lane) {
                spans_into(&out, lane, &codes[c].unwrap(), &maps[c].unwrap(), proj.rows, &mut cache[lane], span);
            }
        }
        black_box(&spans);
    });
    eprintln!(
        "{N}x{n} mask={mask:04b} density={density:5.2} corrected={} ns/word kernel={kernel:.3} edges={column_edges:.3} full={full:.3} old={old:.3} speedup={:.2}x/{:.2}x",
        record.offsets.is_some(),
        old / kernel,
        old / full
    );
}

fn ns_per_sample(samples: usize, mut work: impl FnMut()) -> f64 {
    const REPEATS: usize = 500;
    for _ in 0..32 {
        work();
    }
    let mut timings: [f64; 5] = std::array::from_fn(|_| {
        let start = Instant::now();
        for _ in 0..REPEATS {
            work();
        }
        start.elapsed().as_nanos() as f64 / (REPEATS * samples) as f64
    });
    timings.sort_by(f64::total_cmp);
    timings[timings.len() / 2]
}

#[test]
#[ignore = "host placement timing, run alone in release"]
fn measured_record_placement() {
    measure_placement::<1>(1, 14064);
    measure_placement::<2>(3, 7032);
    measure_placement::<4>(15, 3520);
}

fn measure_placement<const N: usize>(mask: u32, len: usize) {
    use crate::drawpipe::route::Route;
    use crate::picture::deposit::standard::{batch_plan_timed, projection, BatchPlacement};
    let mut model = crate::model::Model::default();
    for (c, channel) in model.ch.iter_mut().enumerate() {
        channel.on = mask & (1 << c) != 0;
    }
    let w = 1080;
    let dt = (model.time_per_div() as f64 * HDIV as f64) / (len as f64 * 0.96);
    let rows: Vec<[u16; N]> = (0..len).map(|i| [if i < len / 2 { 26000 } else { 39000 }; N]).collect();
    let record = Record::new(mask, rows, Some([[12; N]; PHASES]));
    let mut layout = crate::drawpipe::CodeLayout::uncorrected(N, record.layout.word, record.layout.affine, volt_code);
    layout.corrected =
        Some(std::array::from_fn(|c| CorrectedCodes { offsets: [12; PHASES], scale: CAL[c].0, offset: CAL[c].1 }));
    let frame = crate::board::Frame { dt, pre: len / 2, ..Default::default() };
    let mut previous = None;
    let mut refine = std::time::Duration::ZERO;
    let mut refine_total = std::time::Duration::ZERO;
    let mut calls = 0u32;
    let ns = ns_per_sample(1, || {
        let projection = projection(&model, w, 480, dt / model.time_per_div() as f64 * w as f64 / HDIV as f64);
        let words = Some((record.bytes.as_slice(), layout, Layout::new(&layout).unwrap()));
        let placement = BatchPlacement { projection, words, route: Route::Dense };
        let plan = batch_plan_timed(&model, black_box(&frame), placement, previous.take(), Some(&mut refine));
        refine_total += refine;
        calls += 1;
        previous = plan.plan.into_raw();
        black_box(&previous);
    });
    eprintln!("{N}x{len}: placement {ns:.0} ns/record, refine {:?}/record", refine_total / calls);
}
