// SPDX-License-Identifier: GPL-2.0-or-later

use super::{fixture_stage, kernel, Projection, NCH, POINTS};
use crate::fastdraw::zoom::{Filter, Window};
use crate::fastdraw::{clip_rows, Codes, Map, Spans};
use std::{cell::RefCell, sync::OnceLock};

const TAPS: usize = 16;
const PHASE_BITS: u32 = 10;
const PHASES: usize = 1 << PHASE_BITS;
const COEFFICIENT_BITS: u32 = 14;
const COEFFICIENT_UNIT: i32 = 1 << COEFFICIENT_BITS;
const POSITION_BITS: u32 = 36;
const POSITION_UNIT: i64 = 1 << POSITION_BITS;
const ROW_BITS: u32 = 12;

type Taps = [i16; TAPS];
type Phases = [Taps; PHASES + 1];

fn kernel(edge: usize) -> &'static Phases {
    static KERNELS: [OnceLock<Phases>; TAPS] = [const { OnceLock::new() }; TAPS];
    KERNELS[edge].get_or_init(|| {
        let sinc = |x: f64| {
            let radians = std::f64::consts::PI * x;
            if x == 0.0 {
                1.0
            } else {
                radians.sin() / radians
            }
        };
        let (start, end) = if edge < 8 { (edge, TAPS) } else { (0, TAPS - (edge - 7)) };
        std::array::from_fn(|phase| {
            let pivot = (7 + usize::from(phase > PHASES / 2)).min(end - 1);
            if edge == TAPS - 1 {
                // At the last captured sample the only used phase is zero.
                let mut taps = [0; TAPS];
                taps[pivot] = COEFFICIENT_UNIT as i16;
                return taps;
            }
            let fraction = phase as f64 / PHASES as f64;
            let mut weights = [0.0; TAPS];
            for (j, w) in weights.iter_mut().enumerate().take(end).skip(start) {
                let x = fraction + 7.0 - j as f64;
                *w = sinc(x) * sinc(x / 8.0);
            }
            let scale = f64::from(COEFFICIENT_UNIT) / weights.iter().sum::<f64>();
            let mut taps = weights.map(|w| (w * scale).round() as i16);
            let residue = COEFFICIENT_UNIT - taps.iter().map(|&w| i32::from(w)).sum::<i32>();
            taps[pivot] += residue as i16;
            taps
        })
    })
}

fn visible_kernels(lo: usize, hi: usize, len: usize) -> [&'static Phases; TAPS] {
    let interior = kernel(0);
    std::array::from_fn(|edge| {
        let sample = if edge < 8 { 7 - edge } else { len - (16 - edge) };
        if edge != 0 && (lo..=hi).contains(&sample) {
            kernel(edge)
        } else {
            interior
        }
    })
}

#[derive(Clone, Copy)]
struct Edge {
    index: usize,
    mix: i32,
}

impl Edge {
    fn row(self, first: usize, values: &[i32]) -> Option<i32> {
        let i = self.index.checked_sub(first)?;
        let (&a, &b) = values.get(i.checked_sub(1)?).zip(values.get(i))?;
        Some(a + ((i64::from(b - a) * i64::from(self.mix)) >> 16) as i32)
    }
}

#[derive(Default)]
struct Scratch {
    geometry: Option<(usize, u64, u64, u64, u64)>,
    edges: Vec<Edge>,
    samples: Vec<i16>,
    vertices: Vec<i32>,
    short: Filter,
}

impl Scratch {
    fn edges(&mut self, p: &Projection, first: f64, step: f64) {
        let key = (p.cols, p.time_per_div.to_bits(), p.delay.to_bits(), first.to_bits(), step.to_bits());
        if self.geometry == Some(key) {
            return;
        }
        self.geometry = Some(key);
        self.edges.clear();
        let ppd = p.cols as f64 / f64::from(crate::model::HDIV);
        let x = |i: usize| (((first + i as f64 * step - p.delay) / p.time_per_div) * ppd + p.cols as f64 / 2.0) as f32;
        let mut index = 0;
        for c in 0..=p.cols {
            let col = c as f32;
            while index < POINTS && x(index) < col {
                index += 1;
            }
            let mix = if (1..POINTS).contains(&index) {
                let a = x(index - 1);
                (((col - a) / (x(index) - a)) * 65_536.0).round() as i32
            } else {
                0
            };
            self.edges.push(Edge { index, mix });
        }
    }
}

thread_local! {
    static SCRATCH: RefCell<Scratch> = RefCell::new(Scratch::default());
}

pub(super) fn place<'a>(
    p: &Projection,
    (first, dt, sample_dt, ti): (f64, f64, f64, f64),
    codes: impl Fn(usize) -> Codes<'a>,
    spans: &mut [Option<Spans>; NCH],
) {
    let origin = position(ti + first / sample_dt);
    let stride = position(dt / sample_dt);
    let lanczos = sample_dt > 2.0 * dt;
    SCRATCH.with_borrow_mut(|scratch| {
        scratch.edges(p, first, dt);
        for (c, span) in spans.iter_mut().enumerate() {
            let codes = codes(c);
            let Some(vertical) = p.ch[c].filter(|_| codes.n >= 2) else {
                *span = None;
                continue;
            };
            let out = span.get_or_insert_with(Spans::default);
            if codes.n < TAPS {
                // Both ends can clip the same tap window. The zoom filter normalises those taps together.
                let window = Window { start: ti + first / sample_dt, step: dt / sample_dt, column_origin: 0.0 };
                fixture_stage!(Resample, c, scratch.short.spans(&codes, window, (p.cols, p.rows), vertical, out));
                continue;
            }
            let Some(map) = kernel::raw_map(p, c, ti) else { continue };
            let row = RowScale::new(&codes, &map);
            fixture_stage!(Resample, c, interpolate(scratch, &codes, (origin, stride), lanczos, &row));
            let first = visible_start(origin, stride);
            fixture_stage!(FusedReduceMap, c, spans_into(&scratch.edges, first, &scratch.vertices, p.rows, out));
        }
    });
}

fn position(samples: f64) -> i64 {
    (samples * POSITION_UNIT as f64).round() as i64
}

fn visible_start(origin: i64, stride: i64) -> usize {
    if origin < 0 && stride > 0 {
        (origin.unsigned_abs().div_ceil(stride as u64) as usize).min(POINTS)
    } else {
        0
    }
}

struct RowScale {
    gain: i64,
    center: i64,
}

impl RowScale {
    fn new(codes: &Codes<'_>, map: &Map) -> Self {
        // Pre-divide the gain for the fixed-point slope, pinned by corrected_samples_use_calibrated_row_gain.
        let gain = codes.corrected.map_or(f64::from(codes.affine.0), |cal| cal.scale / 7500.0);
        let scale = f64::from(map.y_per_volt) * gain;
        let center = f64::from(codes.volts(32768)) * f64::from(map.y_per_volt) + f64::from(map.y0);
        Self { gain: (scale * (1u64 << 27) as f64).round() as i64, center: (center * 4096.0).round() as i64 }
    }

    #[inline]
    fn row(&self, code: i32) -> i32 {
        // Q14 code times Q27 slope, rounded to Q12 rows. Leave headroom for edge interpolation.
        let scaled = self.gain.saturating_mul(i64::from(code));
        let row = (scaled >> (COEFFICIENT_BITS + 27 - ROW_BITS)).saturating_add(self.center);
        row.clamp(-(1 << 29), 1 << 29) as i32
    }
}

fn interpolate(s: &mut Scratch, codes: &Codes<'_>, (origin, stride): (i64, i64), lanczos: bool, scale: &RowScale) {
    let first = visible_start(origin, stride);
    let last = position((codes.n - 1) as f64);
    s.vertices.clear();
    if stride <= 0 || first == POINTS || origin.saturating_add(stride.saturating_mul(first as i64)) > last {
        return;
    }
    let start = origin + stride * first as i64;
    let end = ((last - start) / stride + 1).min((POINTS - first) as i64) as usize;
    let lo = (start >> POSITION_BITS) as usize;
    let hi = ((start + stride * (end - 1) as i64) >> POSITION_BITS) as usize;
    let a = lo.saturating_sub(7);
    let b = (hi + 9).min(codes.n);
    s.samples.clear();
    if lanczos {
        s.samples.extend([0; 7]);
    }
    s.samples.extend((a..b).map(|i| (codes.at_code(i) ^ 0x8000) as i16));
    if lanczos {
        s.samples.extend([0; 8]);
        let kernels = visible_kernels(lo, hi, codes.n);
        s.vertices.extend((0..end).map(|k| {
            let x = start + stride * k as i64;
            let i = (x >> POSITION_BITS) as usize;
            let phase = rounded_phase(x, PHASE_BITS);
            let edge = if i + 8 >= codes.n { 8 + i + 8 - codes.n } else { 7usize.saturating_sub(i) };
            let samples = s.samples[i - a..][..TAPS].first_chunk::<TAPS>().expect("the sample window is padded");
            scale.row(dot(samples, &kernels[edge][phase]))
        }));
    } else {
        s.vertices.extend((0..end).map(|k| {
            let x = start + stride * k as i64;
            let i = (x >> POSITION_BITS) as usize - a;
            let left = i32::from(s.samples[i]);
            let right = i32::from(s.samples.get(i + 1).copied().unwrap_or(s.samples[i]));
            let fraction = rounded_phase(x, COEFFICIENT_BITS) as i32;
            scale.row(left * COEFFICIENT_UNIT + (right - left) * fraction)
        }));
    }
}

#[inline]
fn rounded_phase(x: i64, bits: u32) -> usize {
    let shift = POSITION_BITS - bits;
    (((x & (POSITION_UNIT - 1)) + (1 << (shift - 1))) >> shift) as usize
}

#[inline]
#[cfg(target_arch = "aarch64")]
fn dot(samples: &Taps, weights: &Taps) -> i32 {
    use core::arch::aarch64::*;
    // SAFETY: aarch64 has NEON and each array contains two full eight-element vectors.
    unsafe {
        let a = vld1q_s16(samples.as_ptr());
        let b = vld1q_s16(weights.as_ptr());
        let c = vld1q_s16(samples.as_ptr().add(8));
        let d = vld1q_s16(weights.as_ptr().add(8));
        let ab = vmlal_high_s16(vmull_s16(vget_low_s16(a), vget_low_s16(b)), a, b);
        let cd = vmlal_high_s16(vmull_s16(vget_low_s16(c), vget_low_s16(d)), c, d);
        vaddvq_s32(vaddq_s32(ab, cd))
    }
}

#[inline]
#[cfg(not(target_arch = "aarch64"))]
fn dot(samples: &Taps, weights: &Taps) -> i32 {
    let products: [i32; TAPS] = std::array::from_fn(|i| i32::from(samples[i]) * i32::from(weights[i]));
    products.into_iter().sum()
}

fn spans_into(edges: &[Edge], first: usize, rows: &[i32], height: usize, out: &mut Spans) {
    out.c0 = 0;
    out.rows.clear();
    if rows.len() < 2 {
        return;
    }
    let index = |edge: Edge| edge.index.saturating_sub(first).min(rows.len());
    let round = |row: i32| (row + (1 << (ROW_BITS - 1))) >> ROW_BITS;
    out.rows.extend(edges.windows(2).map(|e| {
        let (left, right) = (e[0], e[1]);
        let vertices = rows[index(left)..index(right)].iter().copied();
        let (low, high) = vertices
            .chain(left.row(first, rows))
            .chain(right.row(first, rows))
            .fold((i32::MAX, i32::MIN), |(low, high), row| (low.min(row), high.max(row)));
        if low <= high {
            clip_rows(round(low), round(high), height as i32)
        } else {
            (1, 0)
        }
    }));
}

#[cfg(test)]
mod tests {
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
                        let expected = crate::drawpipe::stage::fixture_bench::legacy_hull(
                            &projection,
                            0,
                            (first_time, dt),
                            &points,
                        );
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
}
