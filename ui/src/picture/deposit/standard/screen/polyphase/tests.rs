// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::{fastdraw::CorrectedCodes, model::Model, picture::deposit::standard};

#[test]
fn corrected_samples_use_calibrated_row_gain() {
    let codes = Codes {
        bytes: &[],
        words: None,
        stride: 1,
        word: 0,
        n: 0,
        affine: (0.0, 0.0),
        corrected: Some(CorrectedCodes { offsets: [0; 8], scale: 7500.0 / 16384.0, offset: 0.25 }),
        volt: |_, _| 0.0,
    };
    let map = Map { ti: 0.0, x_per_sample: 1.0, x0: 0.0, y_per_volt: 64.0, y0: 128.0 };
    let row = RowScale::new(&codes, &map);
    assert_eq!(row.row(8192 * COEFFICIENT_UNIT), 144 << ROW_BITS);
}

#[test]
fn fixed_spans_track_the_exact_screen_grid_within_one_row() {
    let m = Model::default();
    let mut scratch = Scratch::default();
    let mut spans = Spans::default();
    for width in [271, 970, 1920] {
        for per in [0.017, 0.043, 0.29, 0.5, 0.71, 1.49] {
            let projection = standard::projection(&m, width, 448, width as f64 / (POINTS as f64 * per));
            let window = projection.time_per_div * f64::from(crate::model::HDIV);
            let dt = window / POINTS as f64;
            let first_time = -window / 2.0;
            let sample_dt = dt / per;
            let volts = |code: u16, _: (f32, f32)| (f32::from(code) - 32768.0) / 16384.0;
            scratch.edges(&projection, first_time, dt);
            for frequency in [0.021, 0.125, 0.297, 0.4] {
                let words = band_limited_words((per * POINTS as f64) as usize + 32, frequency);
                let decoded: Vec<_> = words.iter().map(|&word| volts(word, (0.0, 0.0))).collect();
                let codes = Codes {
                    bytes: &[],
                    words: Some(&words),
                    stride: 1,
                    word: 0,
                    n: words.len(),
                    affine: (1.0 / 16384.0, 0.0),
                    corrected: None,
                    volt: volts,
                };
                for start in [-9.71, 0.0, 12.433, 29.79] {
                    let ti = start - first_time / sample_dt;
                    let row = RowScale::new(&codes, &kernel::raw_map(&projection, 0, ti).unwrap());
                    let geometry = (position(start), position(per));
                    let lanczos = per < 0.5;
                    interpolate(&mut scratch, &codes, geometry, lanczos, &row);
                    spans_into(
                        &scratch.edges,
                        visible_start(geometry.0, geometry.1),
                        &scratch.vertices,
                        projection.rows,
                        &mut spans,
                    );
                    let points: Vec<_> = (0..POINTS)
                        .map(|k| {
                            let x = ti + (first_time + k as f64 * dt) / sample_dt;
                            if lanczos {
                                oos_dsp::interp::lanczos8(&decoded, x)
                            } else {
                                oos_dsp::interp::linear(&decoded, x)
                            }
                            .unwrap_or(f32::NAN)
                        })
                        .collect();
                    let expected =
                        crate::drawpipe::stage::fixture_bench::legacy_hull(&projection, 0, (first_time, dt), &points);
                    assert_hulls(&spans.rows, &expected, (width, per, frequency, start));
                }
            }
        }
    }
}

fn band_limited_words(len: usize, frequency: f64) -> Vec<u16> {
    (0..len)
        .map(|i| {
            let phase = std::f64::consts::TAU * frequency * i as f64;
            (32768.0 + 15500.0 * phase.sin() + 9000.0 * (phase / 3.0 + 1.2).sin()) as u16
        })
        .collect()
}

fn assert_hulls(actual: &[(i16, i16)], expected: &[(i16, i16)], case: impl std::fmt::Debug) {
    assert_eq!(actual.len(), expected.len());
    for (column, (&(a, b), &(c, d))) in actual.iter().zip(expected).enumerate() {
        if a <= b && c <= d {
            assert!(a.abs_diff(c) <= 1 && b.abs_diff(d) <= 1, "{case:?} col={column}: {a}..{b} vs {c}..{d}");
        } else {
            assert_eq!(a <= b, c <= d, "{case:?} col={column}: {a}..{b} vs {c}..{d}");
        }
    }
}

#[test]
fn sample_runs_and_crossings_keep_the_point_at_a_time_result() {
    let mut scratch = Scratch::default();
    let mut spans = Spans::default();
    let words = band_limited_words(207, 0.4);
    let scale = RowScale { gain: -917504, center: 224 << ROW_BITS };
    let codes = Codes {
        bytes: &[],
        words: Some(&words),
        stride: 1,
        word: 0,
        n: words.len(),
        affine: (1.0 / 16384.0, 0.0),
        corrected: None,
        volt: |code, _| (f32::from(code) - 32768.0) / 16384.0,
    };
    for width in [271, 970, 1920] {
        let projection = standard::projection(&Model::default(), width, 448, 1.0);
        let window = projection.time_per_div * f64::from(crate::model::HDIV);
        scratch.edges(&projection, -window / 2.0, window / POINTS as f64);
        for start in [-200.11, -9.71, 0.0, 7.0, 12.433, 205.777, 207.0] {
            for stride in [0.000_434, 0.017, 0.043, 0.29, 0.499, 0.5, 0.71, 1.49] {
                let (origin, step) = (position(start), position(stride));
                let lanczos = stride < 0.5;
                let vertices = point_reference(&words, (origin, step), lanczos, &scale);
                interpolate(&mut scratch, &codes, (origin, step), lanczos, &scale);
                assert_eq!(scratch.vertices, vertices, "{width}/{start}/{stride}");
                let first = visible_start(origin, step);
                spans_into(&scratch.edges, first, &scratch.vertices, projection.rows, &mut spans);
                let expected = column_reference(&scratch.edges, first, &vertices, projection.rows);
                assert_eq!(spans.rows, expected, "{width}/{start}/{stride}");
            }
        }
    }
    let (allocations, ()) = crate::system::test_allocator::count_own(|| {
        let geom = (position(-9.71), position(0.043));
        interpolate(&mut scratch, &codes, geom, true, &scale);
        spans_into(&scratch.edges, visible_start(geom.0, geom.1), &scratch.vertices, 448, &mut spans);
    });
    assert_eq!(allocations, 0);
}

fn point_reference(words: &[u16], (origin, step): (i64, i64), lanczos: bool, scale: &RowScale) -> Vec<i32> {
    let raw = |i: usize| (words[i] ^ 0x8000) as i16;
    let last = position((words.len() - 1) as f64);
    (0..POINTS)
        .map(|k| origin + step * k as i64)
        .filter(|x| (0..=last).contains(x))
        .map(|x| {
            let i = (x >> POSITION_BITS) as usize;
            let code = if lanczos {
                let samples =
                    std::array::from_fn(|tap| (i + tap).checked_sub(7).filter(|&j| j < words.len()).map_or(0, raw));
                let edge = if i + 8 >= words.len() { i + 16 - words.len() } else { 7usize.saturating_sub(i) };
                let weights = &kernel(edge)[rounded_phase(x, PHASE_BITS)];
                crate::picture::deposit::interpolation::scalar_dot(&samples, weights)
            } else {
                let (a, b) = (i32::from(raw(i)), i32::from(raw((i + 1).min(words.len() - 1))));
                a * COEFFICIENT_UNIT + (b - a) * rounded_phase(x, COEFFICIENT_BITS) as i32
            };
            scale.row(code)
        })
        .collect()
}

fn column_reference(edges: &[Edge], first: usize, rows: &[i32], height: usize) -> Vec<(i16, i16)> {
    if rows.len() < 2 {
        return Vec::new();
    }
    let index = |edge: Edge| edge.index.saturating_sub(first).min(rows.len());
    let crossing = |edge: Edge| {
        let i = edge.index.checked_sub(first)?;
        let (&a, &b) = rows.get(i.checked_sub(1)?).zip(rows.get(i))?;
        Some(a + ((i64::from(b - a) * i64::from(edge.mix)) >> 16) as i32)
    };
    let round = |row: i32| (row + (1 << (ROW_BITS - 1))) >> ROW_BITS;
    edges
        .array_windows()
        .map(|&[left, right]| {
            let (low, high) = rows[index(left)..index(right)]
                .iter()
                .copied()
                .chain(crossing(left))
                .chain(crossing(right))
                .fold((i32::MAX, i32::MIN), |(lo, hi), row| (lo.min(row), hi.max(row)));
            if low <= high {
                clip_rows(round(low), round(high), height as i32)
            } else {
                (1, 0)
            }
        })
        .collect()
}

#[test]
fn batched_row_projection_matches_scalar_with_saturation_and_tails() {
    let codes = [i32::MIN, -400_112, -1, 0, 1, 390_931, i32::MAX, -512_117, -102_541];
    for gain in [i64::MIN, i64::from(i32::MIN), -917504, 0, i64::from(i32::MAX), i64::MAX] {
        for center in [i64::MIN, -(1 << 29), 224 << ROW_BITS, 1 << 29, i64::MAX] {
            let scale = RowScale { gain, center };
            let mut actual = [0; 9];
            scale.fill(&mut actual, |k| codes[k]);
            assert_eq!(actual, codes.map(|code| scale.row(code)), "gain={gain}, center={center}");
        }
    }
}
