// SPDX-License-Identifier: GPL-2.0-or-later

use super::{fixture_stage, kernel, Projection, NCH, POINTS};
use crate::fastdraw::zoom::{Filter, Window};
use crate::fastdraw::{clip_rows, Codes, Map, Spans};
use crate::picture::deposit::interpolation::{dot, fill_sample_runs, Taps};
use std::{cell::RefCell, sync::OnceLock};

mod average;
pub(crate) use average::AverageGrid;

const TAPS: usize = 16;
const PHASE_BITS: u32 = 10;
const PHASES: usize = 1 << PHASE_BITS;
const COEFFICIENT_BITS: u32 = 14;
const COEFFICIENT_UNIT: i32 = 1 << COEFFICIENT_BITS;
const POSITION_BITS: u32 = 36;
const POSITION_UNIT: i64 = 1 << POSITION_BITS;
const ROW_BITS: u32 = 12;
const ROW_SHIFT: i32 = (COEFFICIENT_BITS + 27 - ROW_BITS) as i32;
const ROW_LIMIT: i32 = 1 << 29;

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

#[inline]
fn kernel_edge(i: usize, len: usize) -> usize {
    if i + 8 >= len {
        8 + i + 8 - len
    } else {
        7usize.saturating_sub(i)
    }
}

#[derive(Clone, Copy)]
struct Edge {
    index: usize,
    mix: i32,
}

impl Edge {
    fn crossing(self, first: usize, values: &[i32]) -> (usize, (i32, i32)) {
        let i = self.index.saturating_sub(first).min(values.len());
        let extent = if i > 0 && i < values.len() {
            let (a, b) = (values[i - 1], values[i]);
            let row = a + ((i64::from(b - a) * i64::from(self.mix)) >> 16) as i32;
            (row, row)
        } else {
            (i32::MAX, i32::MIN)
        };
        (i, extent)
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
        // Q14 code times Q27 slope, shifted to Q12 rows. Leave headroom for edge interpolation.
        let scaled = self.gain.saturating_mul(i64::from(code));
        let row = (scaled >> ROW_SHIFT).saturating_add(self.center);
        row.clamp(-i64::from(ROW_LIMIT), i64::from(ROW_LIMIT)) as i32
    }

    #[inline]
    fn fill(&self, out: &mut [i32], code: impl Fn(usize) -> i32) {
        #[cfg(not(target_arch = "aarch64"))]
        let first = 0;
        #[cfg(target_arch = "aarch64")]
        let (first, out) = if let Ok(gain) = i32::try_from(self.gain) {
            let (groups, tail) = out.as_chunks_mut::<4>();
            for (i, group) in groups.iter_mut().enumerate() {
                let values = std::array::from_fn(|k| code(4 * i + k));
                self.project_four(values, gain, group);
            }
            (groups.len() * 4, tail)
        } else {
            (0, out)
        };
        for (k, row) in out.iter_mut().enumerate() {
            *row = self.row(code(first + k));
        }
    }

    #[inline]
    #[cfg(target_arch = "aarch64")]
    fn project_four(&self, values: [i32; 4], gain: i32, out: &mut [i32; 4]) {
        use core::arch::aarch64::*;
        // SAFETY: NEON is baseline on aarch64. The input and output arrays each hold a full vector.
        // A product of two i32 values fits in i64. Saturating the narrowing before clipping
        // preserves the scalar clip because its limits are inside the i32 range.
        unsafe {
            let values = vld1q_s32(values.as_ptr());
            let gain = vdupq_n_s32(gain);
            let center = vdupq_n_s64(self.center);
            let low = vmull_s32(vget_low_s32(values), vget_low_s32(gain));
            let high = vmull_high_s32(values, gain);
            let shift = |v| vqaddq_s64(vshrq_n_s64::<ROW_SHIFT>(v), center);
            let row = vqmovn_high_s64(vqmovn_s64(shift(low)), shift(high));
            let row = vmaxq_s32(vdupq_n_s32(-ROW_LIMIT), vminq_s32(row, vdupq_n_s32(ROW_LIMIT)));
            vst1q_s32(out.as_mut_ptr(), row);
        }
    }
}

fn interpolate(s: &mut Scratch, codes: &Codes<'_>, (origin, stride): (i64, i64), lanczos: bool, scale: &RowScale) {
    let first = visible_start(origin, stride);
    let last = position((codes.n - 1) as f64);
    if stride <= 0 || first == POINTS || origin.saturating_add(stride.saturating_mul(first as i64)) > last {
        s.vertices.clear();
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
        fill_sample_runs::<POSITION_BITS>(&mut s.vertices, (start, stride, end), |i, x, out| {
            let edge = kernel_edge(i, codes.n);
            let samples = s.samples[i - a..].first_chunk().expect("the sample window is padded");
            let phases = &kernels[edge];
            scale.fill(out, |k| {
                let phase = rounded_phase(x + stride * k as i64, PHASE_BITS);
                dot(samples, &phases[phase])
            });
        });
    } else {
        // At the final sample the two-tap kernel repeats that sample.
        let tail = *s.samples.last().expect("the visible window contains a sample");
        s.samples.push(tail);
        s.vertices.resize(end, 0);
        scale.fill(&mut s.vertices, |k| {
            let x = start + stride * k as i64;
            let i = (x >> POSITION_BITS) as usize - a;
            let &[left, right] = s.samples[i..].first_chunk().expect("the linear sample window is padded");
            let (left, right) = (i32::from(left), i32::from(right));
            let fraction = rounded_phase(x, COEFFICIENT_BITS) as i32;
            left * COEFFICIENT_UNIT + (right - left) * fraction
        });
    }
}

#[inline]
fn rounded_phase(x: i64, bits: u32) -> usize {
    let shift = POSITION_BITS - bits;
    (((x & (POSITION_UNIT - 1)) + (1 << (shift - 1))) >> shift) as usize
}

fn spans_into(edges: &[Edge], first: usize, rows: &[i32], height: usize, out: &mut Spans) {
    out.c0 = 0;
    out.rows.clear();
    if rows.len() < 2 {
        return;
    }
    let mut crossings = edges.iter().map(|edge| edge.crossing(first, rows));
    let Some(mut left) = crossings.next() else { return };
    let round = |row: i32| (row + (1 << (ROW_BITS - 1))) >> ROW_BITS;
    out.rows.extend(crossings.map(|right| {
        let (start, (a, b)) = std::mem::replace(&mut left, right);
        let (end, (c, d)) = right;
        let (low, high) =
            rows[start..end].iter().fold((a.min(c), b.max(d)), |(lo, hi), &row| (lo.min(row), hi.max(row)));
        if low <= high {
            clip_rows(round(low), round(high), height as i32)
        } else {
            (1, 0)
        }
    }));
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod bench;
