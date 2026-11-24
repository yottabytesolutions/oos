// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::board::{display_record, Frame};
use crate::fastdraw::{self, CorrectedCodes};
use crate::model::{AcqMode, Model, NCH};
use crate::picture::deposit::envelope::adapt::boxcar::{Boxcar, ScreenPoints};
use crate::picture::deposit::envelope::Correction;

fn record<const N: usize>(len: usize, corrected: bool) -> (Vec<[u16; N]>, [Codes<'static>; N], Frame) {
    let offsets = std::array::from_fn(|phase| std::array::from_fn(|lane| 61 * (phase as i32 - 4) + lane as i32));
    let correction = Correction::new(offsets);
    let mut random = 12345u32;
    let words: Vec<[u16; N]> = (0..len)
        .map(|i| {
            let row = std::array::from_fn(|_| {
                random = random.wrapping_mul(1664525).wrapping_add(1013904223);
                (random >> 16) as u16
            });
            if corrected {
                correction.row(row, i)
            } else {
                row
            }
        })
        .collect();
    let cal: [Codes<'_>; N] = std::array::from_fn(|lane| {
        let scale = 1.203 - lane as f64 * 0.1;
        let offset = -0.1337 + lane as f64 * 0.2;
        let mut codes = calibration(((scale / 7500.0) as f32, offset as f32));
        codes.corrected = corrected.then(|| CorrectedCodes { offsets: offsets.map(|row| row[lane]), scale, offset });
        codes
    });
    let ch = std::array::from_fn(|lane| {
        cal.get(lane)
            .map(|c| words.iter().map(|row| c.calibrated(row[lane], row[lane]).0).collect())
            .unwrap_or_default()
    });
    let frame = Frame { dt: 0.8e-9 * N as f64, ch, ..Default::default() };
    (words, cal, frame)
}

fn compare<const N: usize>() {
    let model = Model { acq: AcqMode::HiRes, ..Model::default() };
    let points = 3600;
    for corrected in [false, true] {
        let (words, cal, mut frame) = record::<N>(22_500, corrected);
        let mut boxcar = Boxcar::with_capacity(points);
        for (density, first) in [(2.0, -20.2), (6.25, -45.8), (6.273, 3.1), (36.7, -40.5)] {
            let window = frame.dt * density * points as f64;
            let dt = window / points as f64;
            let first = first * frame.dt;
            let ti = 5.37;
            frame.fixed_ti = Some(ti);
            let legacy = display_record(&frame, &model, points, window, first);
            let grid = ScreenPoints::new(first, dt, frame.dt, ti, points);
            assert!(boxcar.filter(&words, &grid));
            for (lane, &calibration) in cal.iter().enumerate() {
                let map = draw_map(points, lane);
                let (mut actual, mut expected) = (Spans::default(), Spans::default());
                {
                    let filtered = boxcar.lane(&words, lane, calibration, map);
                    fastdraw::spans_mapped(
                        WIDTH,
                        HEIGHT,
                        crate::fastdraw::Samples::len(&filtered),
                        &map,
                        &mut actual,
                        |a, b| {
                            (
                                crate::fastdraw::Samples::extremes(&filtered, a, b),
                                a.checked_sub(1).map(|i| crate::fastdraw::Samples::at(&filtered, i)),
                            )
                        },
                    );
                }
                fastdraw::spans_into(WIDTH, HEIGHT, legacy.ch[lane].as_slice(), &map, &mut expected);
                assert_spans(&actual, &expected);
                boxcar.lane(&words, lane, calibration, map).spans(
                    (WIDTH, HEIGHT),
                    &mut Default::default(),
                    &mut actual,
                );
                assert_spans(&actual, &expected);
            }
        }
    }
}

fn draw_map(points: usize, lane: usize) -> Map {
    Map {
        ti: 0.0,
        x_per_sample: WIDTH as f64 / points as f64,
        x0: 0.0,
        y_per_volt: -56.0 * (1 + lane) as f32,
        y0: HEIGHT as f32 / 2.0 + 0.37,
    }
}

#[test]
fn boxcar_matches_decoded_hires_on_one_two_and_four_channels() {
    assert_eq!(NCH, 4);
    compare::<1>();
    compare::<2>();
    compare::<4>();
}

#[test]
fn boxcar_preserves_rounded_decode_at_a_pixel_boundary() {
    let words = [[65535], [65535], [65535]];
    let cal = calibration((0.0001384621, -0.1161));
    let exact = cal.calibrated(65535, 65535).0;
    let map = Map { ti: 0.0, x_per_sample: 1.0, x0: 0.0, y_per_volt: 1.0, y0: 4.5 - exact };
    let mut boxcar = Boxcar::with_capacity(1);
    assert!(!boxcar.filter(&words, &ScreenPoints::new(0.0, 1.99, 1.0, 0.0, 1)));
    assert!(boxcar.filter(&words, &ScreenPoints::new(0.0, 3.0, 1.0, 0.0, 1)));
    let mut filtered = boxcar.lane(&words, 0, cal, map);
    assert_eq!(fastdraw::Samples::at(&filtered, 0), exact);
    let mut spans = Spans::default();
    filtered.spans((1, 16), &mut Default::default(), &mut spans);
    assert_eq!(spans.rows, [(5, 5)]);
}

#[test]
#[ignore = "host timing comparison, set OOS_PERF_TESTS=1 and run alone in release"]
fn measured_boxcar() {
    let (words, cal, mut frame) = record::<4>(100_000, true);
    let model = Model { acq: AcqMode::HiRes, ..Model::default() };
    let points = 3600;
    let window = words.len() as f64 * frame.dt;
    let first = -window / 2.0;
    let trigger = words.len() as f64 / 2.0;
    frame.fixed_ti = Some(trigger);
    let grid = ScreenPoints::new(first, window / points as f64, frame.dt, trigger, points);
    let mut boxcar = Boxcar::with_capacity(points);
    let mut spans = Spans::default();
    let old = bench(words.len() * 4, || {
        let record = display_record(black_box(&frame), &model, points, window, first);
        for (lane, values) in record.ch.iter().enumerate() {
            fastdraw::spans_into(WIDTH, HEIGHT, values.as_slice(), &draw_map(points, lane), &mut spans);
            black_box(&spans);
        }
    });
    let new = bench(words.len() * 4, || {
        boxcar.filter(black_box(&words), &grid);
        for (lane, &cal) in cal.iter().enumerate() {
            let map = draw_map(points, lane);
            {
                let filtered = boxcar.lane(&words, lane, cal, map);
                fastdraw::spans_mapped(
                    WIDTH,
                    HEIGHT,
                    crate::fastdraw::Samples::len(&filtered),
                    &map,
                    &mut spans,
                    |a, b| {
                        (
                            crate::fastdraw::Samples::extremes(&filtered, a, b),
                            a.checked_sub(1).map(|i| crate::fastdraw::Samples::at(&filtered, i)),
                        )
                    },
                );
            }
            black_box(&spans);
        }
    });
    println!("HiRes 4 x 100k: {old:.3} -> {new:.3} ns/input word including row mapping");
}
