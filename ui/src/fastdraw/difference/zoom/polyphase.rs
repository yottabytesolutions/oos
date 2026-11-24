// SPDX-License-Identifier: GPL-2.0-or-later

use crate::fastdraw::{clip_rows, Codes, Spans};
use crate::model::VDIV;
use crate::picture::deposit::kernel::Vertical;

const PHASE_BITS: u32 = 10;
const PHASES: usize = 1 << PHASE_BITS;
const TAPS: usize = 16;
const LEFT: usize = TAPS / 2 - 1;
const WEIGHT_BITS: u32 = 14;
const WEIGHT_ONE: i32 = 1 << WEIGHT_BITS;
const SAMPLE_BITS: u32 = 32;
const SAMPLE_ONE: i64 = 1 << SAMPLE_BITS;
const COLUMN_BITS: u32 = 24;
const COLUMN_ONE: i64 = 1 << COLUMN_BITS;

#[derive(Default)]
pub(crate) struct Filter {
    phases: Vec<[i16; TAPS]>,
    support: Vec<i16>,
    points: Vec<i32>,
}

/// The window in raw sample coordinates and its screen lattice in column coordinates.
#[derive(Clone, Copy)]
pub(crate) struct Window {
    pub(crate) start: f64,
    pub(crate) step: f64,
    pub(crate) column_origin: f64,
}

impl Filter {
    pub(crate) fn spans(
        &mut self,
        codes: &Codes<'_>,
        window: Window,
        size: (usize, usize),
        vertical: Vertical,
        out: &mut Spans,
    ) {
        let (width, height) = size;
        out.c0 = 0;
        out.rows.clear();
        if codes.n < 2 || width == 0 {
            return;
        }
        let points = crate::sparse::POINTS;
        let start = (window.start * SAMPLE_ONE as f64).round() as i64;
        let step = (window.step * SAMPLE_ONE as f64).round() as i64;
        let last = (codes.n as i64 - 1).saturating_mul(SAMPLE_ONE);
        let step = step.max(1);
        let project = RowProjection::new(codes, vertical, height);
        self.points.clear();
        self.points.reserve(points + 1);
        if window.column_origin == 0.0 && self.column_spans(codes, (start, step), size, &project, &mut out.rows) {
            return;
        }
        let index = |delta: i64| (delta.div_euclid(step).max(0) as usize).min(points);
        let first = index(start.saturating_neg().saturating_add(step - 1));
        let end = index(last.saturating_sub(start).saturating_add(step));
        if end.saturating_sub(first) < 2 {
            return;
        }
        let start = start + step * first as i64;
        if window.step < 0.5 {
            self.interpolate(codes, (start, step, start + step * (end - first - 1) as i64), end - first);
        } else {
            self.points.extend((0..end - first).map(|i| linear(codes, start + step * i as i64)));
        }
        let scale = points as f64 / width as f64;
        let edge = ((-window.column_origin * scale - first as f64) * COLUMN_ONE as f64).round() as i64;
        let advance = (scale * COLUMN_ONE as f64).round() as i64;
        columns(&self.points, (edge, advance), size, &project, &mut out.rows);
    }

    fn column_spans(
        &mut self,
        codes: &Codes<'_>,
        (start, step): (i64, i64),
        (width, height): (usize, usize),
        project: &RowProjection,
        out: &mut Vec<(i16, i16)>,
    ) -> bool {
        let knots = crate::sparse::POINTS;
        let advance = (step * knots as i64 + width as i64 / 2) / width as i64;
        let end = start.saturating_add(step * (knots - 1) as i64);
        let (first, last) = ((start >> SAMPLE_BITS) - LEFT as i64, (end >> SAMPLE_BITS) + (TAPS - LEFT) as i64);
        if width > knots || advance > SAMPLE_ONE / 32 || first < 0 || last > codes.n as i64 {
            return false;
        }
        let (low, high) = codes.code_extremes(first as usize, last as usize);
        // The Lanczos-8 phase and curvature errors together stay below a row for this
        // spacing and input range. Wider or steeper windows retain every legacy knot.
        let spread = project.slope.saturating_abs().saturating_mul(i64::from(high - low));
        if spread > (128 << (30 - WEIGHT_BITS)) {
            return false;
        }
        self.interpolate(codes, (start, advance, end), width + 1);
        let mut previous = project.row(self.points[0]);
        out.extend(self.points[1..].iter().map(|&code| {
            let next = project.row(code);
            let first = std::mem::replace(&mut previous, next);
            clip_rows(first.min(next), first.max(next), height as i32)
        }));
        true
    }

    fn interpolate(&mut self, codes: &Codes<'_>, (start, step, last_knot): (i64, i64, i64), count: usize) {
        if self.phases.is_empty() {
            self.phases.extend((0..=PHASES).map(taps));
        }
        let base = (start >> SAMPLE_BITS) as usize;
        let end = (last_knot >> SAMPLE_BITS) as usize;
        self.support.clear();
        // Pad so each interior evaluation borrows one fixed-size array, including at the record ends.
        let first = base.saturating_sub(LEFT);
        let last = (end + TAPS - LEFT).min(codes.n);
        self.support.reserve(((step * crate::sparse::POINTS as i64) >> SAMPLE_BITS) as usize + 2 * TAPS);
        let prefix = LEFT.saturating_sub(base);
        self.support.resize(prefix, 0);
        self.support.extend((first..last).map(|i| (i32::from(codes.at_code(i)) - 32_768) as i16));
        let valid = prefix..self.support.len();
        self.support.resize(end - base + TAPS, 0);
        const PHASE_SHIFT: u32 = SAMPLE_BITS - PHASE_BITS;
        let support = self.support.as_slice();
        let phases = self.phases.as_slice();
        let input = |i: usize| {
            let coordinate = (start + step * i as i64).min(last_knot);
            let index = (coordinate >> SAMPLE_BITS) as usize - base;
            let phase = (((coordinate & (SAMPLE_ONE - 1)) + (1 << (PHASE_SHIFT - 1))) >> PHASE_SHIFT) as usize;
            let samples = support[index..].first_chunk().expect("support is padded for all taps");
            (index, samples, &phases[phase])
        };
        if valid == (0..support.len()) {
            self.points.extend((0..count).map(|i| {
                let (_, samples, weights) = input(i);
                dot(samples, weights)
            }));
        } else {
            self.points.extend((0..count).map(|i| {
                let (index, samples, weights) = input(i);
                let range = valid.start.saturating_sub(index)..(valid.end - index).min(TAPS);
                let sum = dot(samples, weights);
                if range.len() == TAPS {
                    sum
                } else {
                    normalise(sum, &weights[range])
                }
            }));
        }
    }
}

fn linear(codes: &Codes<'_>, coordinate: i64) -> i32 {
    // The exact screen uses its two-tap kernel when consecutive knots are at least half a sample apart.
    let index = (coordinate >> SAMPLE_BITS) as usize;
    let a = i64::from(codes.at_code(index)) - 32_768;
    let b = i64::from(codes.at_code((index + 1).min(codes.n - 1))) - 32_768;
    let fraction = coordinate & (SAMPLE_ONE - 1);
    ((a << WEIGHT_BITS) + (((b - a) * fraction) >> (SAMPLE_BITS - WEIGHT_BITS))) as i32
}

fn taps(phase: usize) -> [i16; TAPS] {
    let fraction = phase as f64 / PHASES as f64;
    let sinc = |x: f64| {
        let angle = x * std::f64::consts::PI;
        if angle == 0.0 {
            1.0
        } else {
            angle.sin() / angle
        }
    };
    let raw: [f64; TAPS] = std::array::from_fn(|tap| {
        let d = fraction - (tap as f64 - LEFT as f64);
        f64::from((sinc(d) * sinc(d / 8.0)) as f32)
    });
    let scale = f64::from(WEIGHT_ONE) / raw.iter().sum::<f64>();
    let mut weights = raw.map(|w| (w * scale).round() as i16);
    let residual = WEIGHT_ONE - weights.iter().copied().map(i32::from).sum::<i32>();
    weights[LEFT + usize::from(phase > PHASES / 2)] += residual as i16;
    weights
}

#[inline]
fn dot(input: &[i16; TAPS], weights: &[i16; TAPS]) -> i32 {
    input.iter().zip(weights).map(|(&code, &weight)| i32::from(code) * i32::from(weight)).sum()
}

#[cold]
fn normalise(value: i32, weights: &[i16]) -> i32 {
    // The surviving taps of a position inside the record sum close to one. Newton iteration
    // computes their reciprocal in Q24 without a division at each clipped screen point.
    const ONE: i64 = 1 << 24;
    let sum = i64::from(weights.iter().copied().map(i32::from).sum::<i32>()) << (24 - WEIGHT_BITS);
    let mut inverse = ONE;
    for _ in 0..4 {
        inverse = (inverse * (2 * ONE - ((sum * inverse) >> 24))) >> 24;
    }
    ((i64::from(value) * inverse) >> 24) as i32
}

struct RowProjection {
    origin: i64,
    slope: i64,
}

impl RowProjection {
    fn new(codes: &Codes<'_>, vertical: Vertical, height: usize) -> Self {
        let (lo, hi) = (f64::from(codes.volts(0)), f64::from(codes.volts(u16::MAX)));
        let rows_per_volt = -(height as f64 / f64::from(VDIV)) / f64::from(vertical.volts_per_div);
        let slope = (hi - lo) / f64::from(u16::MAX);
        let zero = height as f64 / 2.0 + (lo + slope * 32_768.0 + f64::from(vertical.offset)) * rows_per_volt;
        let unit = (1u64 << 30) as f64;
        Self {
            origin: (zero * unit).round() as i64,
            slope: (slope * rows_per_volt * (unit / f64::from(WEIGHT_ONE))).round() as i64,
        }
    }

    #[inline]
    fn row(&self, code: i32) -> i32 {
        let v = self.origin.saturating_add(self.slope.saturating_mul(i64::from(code)));
        ((v.saturating_add((1 << 29) - i64::from(v < 0))) >> 30).clamp(i32::MIN as i64, i32::MAX as i64) as i32
    }
}

fn columns(
    points: &[i32],
    (mut edge, step): (i64, i64),
    (width, height): (usize, usize),
    project: &RowProjection,
    out: &mut Vec<(i16, i16)>,
) {
    let last = (points.len() as i64 - 1) * COLUMN_ONE;
    let mut previous = crossing(points, edge.clamp(0, last));
    out.extend((0..width).map(|_| {
        let left = edge;
        edge = edge.saturating_add(step);
        if edge <= 0 || left > last {
            return (1, 0);
        }
        let left = left.max(0);
        let right = edge.min(last);
        let next = crossing(points, right);
        let begin = std::mem::replace(&mut previous, next);
        let lo = ((left + COLUMN_ONE - 1) >> COLUMN_BITS) as usize;
        let hi = (right >> COLUMN_BITS) as usize + 1;
        let (a, b) =
            points[lo.min(hi)..hi].iter().fold((begin.min(next), begin.max(next)), |(a, b), &v| (a.min(v), b.max(v)));
        let (a, b) = (project.row(a), project.row(b));
        clip_rows(a.min(b), a.max(b), height as i32)
    }));
}

#[inline]
fn crossing(points: &[i32], edge: i64) -> i32 {
    let i = (edge >> COLUMN_BITS) as usize;
    let a = points[i];
    let b = points.get(i + 1).copied().unwrap_or(a);
    (i64::from(a) + (((i64::from(b) - i64::from(a)) * (edge & (COLUMN_ONE - 1))) >> COLUMN_BITS)) as i32
}

#[cfg(test)]
mod tests;
