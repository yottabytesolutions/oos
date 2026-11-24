// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::picture::deposit::envelope::adapt::pairs;
use crate::picture::deposit::kernel::SamplesRef;

fn place(
    input: SamplesRef<'_>,
    edges: &mut ColumnEdges,
    map: &Map,
    (width, height): (usize, usize),
    dots: bool,
    spans: &mut Spans,
) {
    if let SamplesRef::Codes(codes) = input {
        let mut cache = RowCache::default();
        let rows = cache.get(&codes, map, height).unwrap();
        edges.update_for(width, codes.n, map, true);
        pairs::code_spans_into(&codes, edges, width, dots, rows, spans);
    } else {
        pairs::spans_into(&input, edges, map, (width, height), dots, spans);
    }
}

fn words(len: usize) -> Vec<u16> {
    (0..len).map(|i| ((i / 2) * 3137 + i % 2 * 4389) as u16).collect()
}

#[test]
fn pairs_match_compression_with_clipping_joins_and_partial_pairs() {
    let mut edges = ColumnEdges::with_capacity(WIDTH);
    let (mut actual, mut expected) = (Spans::default(), Spans::default());
    for len in [0, 1, 2, 17, 6001, 8192] {
        let words = words(len);
        for slope in [1.0, -1.0] {
            let calibration = calibration((slope / 7500.0, 0.4));
            let decoded: Vec<_> = words.iter().map(|&c| calibration.calibrated(c, c).0).collect();
            let codes = Codes { words: Some(&words), n: words.len(), ..calibration };
            let inputs = [SamplesRef::Volts(&decoded), SamplesRef::Codes(codes)];
            for (x_per_sample, ti, x0) in [(0.17, 315.8, 70.1), (0.5, 20.0, 0.5), (0.53, 16.7, 5.9), (0.0, 0.0, 0.0)] {
                let map = Map { ti, x_per_sample, x0, y0: 182.2, y_per_volt: -80.0 };
                for dots in [false, true] {
                    crate::compress::reference_spans(WIDTH, HEIGHT, &decoded, &map, dots, &mut expected);
                    for input in inputs {
                        place(input, &mut edges, &map, (WIDTH, HEIGHT), dots, &mut actual);
                        assert_spans(&actual, &expected);
                    }
                }
            }
        }
    }
}

#[test]
fn random_pairs_match_pre_adapter_spans_for_lines_and_dots() {
    let mut seed = 0xfac3_0e19_u32;
    let mut random = || {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        f64::from(seed) / f64::from(u32::MAX)
    };
    let mut edges = ColumnEdges::with_capacity(WIDTH);
    let (mut actual, mut expected) = (Spans::default(), Spans::default());
    for case in 0..512 {
        let len = (random() * 8193.0) as usize;
        let words: Vec<_> = (0..len).map(|_| (random() * 65536.0) as u16).collect();
        let calibration = calibration((((random() - 0.5) / 3750.0) as f32, (random() - 0.5) as f32));
        let decoded: Vec<_> = words.iter().map(|&code| calibration.calibrated(code, code).0).collect();
        let codes = Codes { words: Some(&words), n: len, ..calibration };
        let width = [4, 97, 384, WIDTH][case % 4];
        let height = [12, 112, 256, HEIGHT][case % 4];
        let x_per_sample = 0.005 + random() * 0.795;
        let ti = random() * len as f64;
        let mut x0 = (random() * 1.5 - 0.25) * width as f64;
        if case % 3 == 0 {
            let centre = 2 * (len / 4) + 1;
            x0 = x0.round() - (centre as f64 - ti) * x_per_sample;
        }
        let map = Map {
            ti,
            x_per_sample,
            x0,
            y0: (random() * height as f64) as f32,
            y_per_volt: (random() * 400.0 - 200.0) as f32,
        };
        for dots in [false, true] {
            crate::compress::reference_spans(width, height, &decoded, &map, dots, &mut expected);
            for input in [SamplesRef::Volts(&decoded), SamplesRef::Codes(codes)] {
                place(input, &mut edges, &map, (width, height), dots, &mut actual);
                assert_eq!(actual.c0, expected.c0);
                assert_eq!(actual.rows, expected.rows, "case {case}, dots={dots}");
            }
        }
    }
}

#[test]
fn an_empty_predecessor_does_not_extend_a_pair_to_the_frame_edges() {
    let samples = [f32::NAN, f32::NAN, 2.0, 3.0];
    let map = Map { ti: 0.0, x_per_sample: 0.5, x0: 0.0, y0: 0.0, y_per_volt: 1.0 };
    let (mut actual, mut expected) = (Spans::default(), Spans::default());
    crate::compress::reference_spans(2, 8, &samples, &map, false, &mut expected);
    pairs::spans_into(samples.as_slice(), &mut ColumnEdges::default(), &map, (2, 8), false, &mut actual);
    assert_spans(&actual, &expected);
}

#[test]
#[ignore = "host timing comparison, set OOS_PERF_TESTS=1 and run alone in release"]
fn measured_pairs() {
    let words = words(4096);
    let calibration = calibration((1.0 / 7500.0, 0.4));
    let decoded: Vec<_> = words.iter().map(|&c| calibration.calibrated(c, c).0).collect();
    let map = Map { ti: 2048.1, x_per_sample: 0.2, x0: 485.0, y0: 203.0, y_per_volt: -56.0 };
    let mut edges = ColumnEdges::with_capacity(WIDTH);
    let mut spans = Spans::default();
    let old = bench(words.len(), || {
        crate::compress::reference_spans(WIDTH, HEIGHT, black_box(&decoded), &map, false, &mut spans);
        black_box(&spans);
    });
    let new = bench(words.len(), || {
        pairs::spans_into(black_box(decoded.as_slice()), &mut edges, &map, (WIDTH, HEIGHT), false, &mut spans);
        black_box(&spans);
    });
    println!("Pairs 4096 words: {old:.3} -> {new:.3} ns/input word including row mapping");
}
