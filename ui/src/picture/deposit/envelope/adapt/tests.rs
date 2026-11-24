// SPDX-License-Identifier: GPL-2.0-or-later

use super::super::{ColumnEdges, Envelopes};
use crate::fastdraw::{code_rows::RowCache, Codes, Map, Spans};
use std::{hint::black_box, time::Instant};

mod boxcar;
mod pairs;

const WIDTH: usize = 970;
const HEIGHT: usize = 448;

fn calibration(affine: (f32, f32)) -> Codes<'static> {
    Codes {
        bytes: &[],
        words: None,
        stride: 1,
        word: 0,
        n: 0,
        affine,
        corrected: None,
        volt: oos_hal::dho::samples::volt_code,
    }
}

fn assert_spans(actual: &Spans, expected: &Spans) {
    assert_eq!(actual.c0, expected.c0);
    assert_eq!(actual.rows, expected.rows);
}

fn bench(samples: usize, mut run: impl FnMut()) -> f64 {
    assert_eq!(std::env::var("OOS_PERF_TESTS").as_deref(), Ok("1"));
    const REPEATS: usize = 500;
    for _ in 0..32 {
        run();
    }
    let mut timings: [f64; 5] = std::array::from_fn(|_| {
        let start = Instant::now();
        for _ in 0..REPEATS {
            run();
        }
        start.elapsed().as_nanos() as f64 / (REPEATS * samples) as f64
    });
    timings.sort_by(f64::total_cmp);
    timings[timings.len() / 2]
}

fn peak_map() -> Map {
    Map { ti: 1759.37, x_per_sample: 0.32, x0: 485.0, y_per_volt: -56.0, y0: 224.0 }
}

#[test]
fn peak_uses_the_same_uncompressed_word_times_as_the_decoder() {
    let words: Vec<u16> = (0..3520).map(|i| (i * 683) as u16).collect();
    let map = peak_map();
    let calibration = calibration((1.0 / 7500.0, 0.3));
    let decoded: Vec<f32> = words.iter().map(|&word| calibration.calibrated(word, word).0).collect();
    let (mut edges, mut envelope) = (ColumnEdges::with_capacity(WIDTH), Envelopes::with_capacity(WIDTH));
    let (mut actual, mut expected) = (Spans::default(), Spans::default());
    edges.update(WIDTH, words.len(), &map);
    super::reduce_lane(&words, &edges, &mut envelope);
    super::super::spans_into(&envelope, 0, &calibration, &map, HEIGHT, &mut Default::default(), &mut actual);
    crate::fastdraw::spans_into(WIDTH, HEIGHT, decoded.as_slice(), &map, &mut expected);
    assert_spans(&actual, &expected);
}

#[test]
#[ignore = "host timing comparison, set OOS_PERF_TESTS=1 and run alone in release"]
fn measured_peak() {
    let words: Vec<u16> = (0..3520).map(|i| (i * 683) as u16).collect();
    let (mut edges, mut envelope) = (ColumnEdges::with_capacity(WIDTH), Envelopes::with_capacity(WIDTH));
    let calibration = calibration((1.0 / 7500.0, 0.3));
    let map = peak_map();
    let mut spans = Spans::default();
    let mut cache = RowCache::default();
    let ns = bench(words.len(), || {
        edges.update(WIDTH, words.len(), black_box(&map));
        super::reduce_lane(black_box(&words), &edges, &mut envelope);
        super::super::spans_into(&envelope, 0, &calibration, &map, HEIGHT, &mut cache, &mut spans);
        black_box(&spans);
    });
    println!("Peak: {ns:.3} ns/input word including envelope and row mapping");
}
