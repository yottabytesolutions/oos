// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::fastdraw::CorrectedCodes;

#[test]
fn four_lane_overview_keeps_corrected_voltage_bits() {
    let words: [[u16; NCH]; 19] =
        std::array::from_fn(|i| std::array::from_fn(|lane| [0, u16::MAX, 32768, (i * 1973) as u16][(i + lane) % NCH]));
    let offsets = [1, -1, 27, -28, 0, 32768, -32768, 8];
    let samples = std::array::from_fn(|lane| Codes {
        bytes: &[],
        words: Some(words.as_flattened()),
        stride: NCH,
        word: lane,
        n: words.len(),
        affine: (0.0, 0.0),
        corrected: Some(CorrectedCodes {
            offsets: std::array::from_fn(|phase| offsets[(phase + lane) % offsets.len()]),
            scale: [0.17, -0.7, 1.0, -1.0][lane],
            offset: [0.031, 0.23, -1.5, 0.0][lane],
        }),
        volt: |_, _| panic!("this fixture requires the corrected calibration"),
    });
    let key = ScreenGrid { ti: 0.0, first: 0.0, dt: 1.0, sample_dt: 1.0, n: 20, len: words.len() };
    let mut grid = EnvelopeGrid::default();
    let mut channels = std::array::from_fn(|_| Vec::new());
    assert!(append(&mut grid, key, &samples, &mut channels));
    for (lane, values) in channels.iter().enumerate() {
        let cal = samples[lane].corrected.unwrap();
        for (pair, range) in grid.ranges.iter().enumerate() {
            assert!(!range.is_empty());
            let mut codes = range
                .clone()
                .map(|i| (i32::from(words[i][lane]) - offsets[(i + lane) % offsets.len()]).clamp(0, 65535));
            let first = codes.next().unwrap();
            let (lo, hi) = codes.fold((first, first), |(lo, hi), code| (lo.min(code), hi.max(code)));
            let volts = |code| ((f64::from(code) - 32768.0) / 7500.0 * cal.scale - cal.offset) as f32;
            let (a, b) = (volts(lo), volts(hi));
            let expected = if a <= b { [a, b] } else { [b, a] };
            assert_eq!([values[2 * pair], values[2 * pair + 1]].map(f32::to_bits), expected.map(f32::to_bits));
        }
    }
}

#[test]
#[ignore = "release microbenchmark for a synthetic four-channel zoom overview"]
fn zoom_overview_microbenchmark() {
    use std::{hint::black_box, time::Instant};

    let len = 31_250;
    let n = crate::sparse::POINTS;
    let words: Vec<u16> = (0..len * NCH).map(|i| (i * 1973) as u16).collect();
    let key = ScreenGrid { ti: 0.381, first: 0.0, dt: len as f64 / n as f64, sample_dt: 1.0, n, len };
    for corrected in [false, true] {
        let samples = std::array::from_fn(|lane| Codes {
            bytes: &[],
            words: Some(&words),
            stride: NCH,
            word: lane,
            n: len,
            affine: (1.0 / 16384.0, 0.0),
            corrected: corrected.then_some(CorrectedCodes {
                offsets: [17, -82, 5, 43, -53, 12, 29, -1],
                scale: 0.25,
                offset: 0.0,
            }),
            volt: |word, _| (f32::from(word) - 32768.0) / 16384.0,
        });
        let mut grid = EnvelopeGrid::default();
        let mut channels = std::array::from_fn(|_| Vec::new());
        let mut run = || {
            assert!(append(&mut grid, black_box(key), black_box(&samples), &mut channels));
            black_box(&channels);
        };
        run();
        let mut elapsed = std::array::from_fn::<_, 9, _>(|_| {
            let start = Instant::now();
            for _ in 0..500 {
                run();
            }
            start.elapsed().as_secs_f64() * 2e6
        });
        elapsed.sort_by(f64::total_cmp);
        let visits: usize = grid.ranges.iter().map(std::ops::Range::len).sum();
        println!("zoom overview: {NCH}x{len}, {n} points/lane, {visits} sample groups visited, corrected={corrected}, {:.0} ns", elapsed[4]);
    }
}
