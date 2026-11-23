// SPDX-License-Identifier: GPL-2.0-or-later
//! On this build the UI selects Normal acquisition in HiRes (`ui/src/board/dho/hw.rs:194`).
//! The HAL has no HiRes decode. The UI averages in `ui/src/board/record.rs:254-282` at all rates
//! with at least two samples per display point (`rate * window / points >= 2`). At lower
//! densities it interpolates just as Normal does. Fractional window boundaries can overlap
//! by a sample. A truncated window divides by its actual count and an empty window is a gap.

use super::super::ColumnEdges;
use crate::drawpipe::metrics::fallback::Cause;
use crate::fastdraw::{self, code_rows, Codes, Map, Samples, Spans};
use std::ops::Range;

const POSITION_BITS: u32 = 32;
const POSITION_UNIT: i64 = 1 << POSITION_BITS;
const MEAN_BITS: u32 = 12;
const RECIP_BITS: u32 = 40;

/// The legacy display grid, with times relative to the trigger and the trigger in sample units.
pub(crate) struct ScreenPoints {
    first: f64,
    dt: f64,
    sample_dt: f64,
    trigger: f64,
    count: usize,
}

impl ScreenPoints {
    pub(crate) fn new(first: f64, dt: f64, sample_dt: f64, trigger: f64, count: usize) -> Self {
        Self { first, dt, sample_dt, trigger, count }
    }

    fn edges(&self, per: f64) -> (i64, i64) {
        let fixed = |x: f64| (x * POSITION_UNIT as f64).round() as i64;
        (fixed(self.trigger + self.first / self.sample_dt), fixed(per))
    }

    fn window(&self, k: usize, per: f64) -> (i64, i64) {
        let center = self.trigger + (self.first + (k as f64 + 0.5) * self.dt) / self.sample_dt;
        ((center - per / 2.0).floor() as i64, (center + per / 2.0).ceil() as i64)
    }
}

struct Sum<const N: usize> {
    words: Range<usize>,
    codes: [u32; N],
    reciprocal: u64,
}

#[derive(Default)]
pub(crate) struct Boxcar<const N: usize> {
    points: Vec<Sum<N>>,
    edges: ColumnEdges,
}

impl<const N: usize> Boxcar<N> {
    #[cfg(test)]
    pub(crate) fn with_capacity(points: usize) -> Self {
        Self { points: Vec::with_capacity(points), edges: ColumnEdges::default() }
    }

    #[cfg(test)]
    pub(crate) fn filter(&mut self, words: &[[u16; N]], grid: &ScreenPoints) -> bool {
        self.filter_with(words.len(), grid, |i| words[i])
    }

    /// Filter already corrected words. Returns false below the HiRes density threshold.
    /// Each output has room for 65,537 full-scale words per lane (125 M / 3,600 < 35,000).
    pub(crate) fn filter_with(&mut self, len: usize, grid: &ScreenPoints, word: impl Fn(usize) -> [u16; N]) -> bool {
        self.points.clear();
        let per = grid.dt / grid.sample_dt;
        if !(2.0..=65_535.0).contains(&per) || len < 2 {
            return false;
        }
        let (mut edge, step) = grid.edges(per);
        let floor = per as usize;
        let reciprocals = [floor, floor + 1, floor + 2].map(reciprocal);
        let mut previous = 0..0;
        let mut last = [0; N];
        let guard = (grid.count as i64 + 2) * 2;
        let near_integer = |edge: i64| (edge.wrapping_add(guard) & (POSITION_UNIT - 1)) <= 2 * guard;
        for k in 0..grid.count {
            let next_edge = edge.saturating_add(step);
            let (start, end) = if near_integer(edge) || near_integer(next_edge) {
                // A sample exactly on a window edge follows the legacy f64 order of operations.
                grid.window(k, per)
            } else {
                (edge >> POSITION_BITS, (next_edge >> POSITION_BITS) + 1)
            };
            edge = next_edge;
            let next = (start.max(0) as usize).min(len)..(end.max(0) as usize).min(len);
            // Adjacent windows share their boundary word. Carry it instead of scanning it twice.
            let shared = !next.is_empty() && next.start + 1 == previous.end;
            let mut sums = if shared { last.map(u32::from) } else { [0; N] };
            for i in next.start + usize::from(shared)..next.end {
                last = word(i);
                for (sum, code) in sums.iter_mut().zip(last) {
                    *sum += u32::from(code);
                }
            }
            let count = next.len();
            let reciprocal = if count == 0 {
                0
            } else {
                reciprocals.get(count.wrapping_sub(floor)).copied().unwrap_or_else(|| reciprocal(count))
            };
            self.points.push(Sum { words: next.clone(), codes: sums, reciprocal });
            previous = next;
        }
        true
    }

    /// Filtered samples for `fastdraw::spans_into` at this row map. The calibration must be the
    /// board's affine f32 decoder, or the f64 corrected decoder carried by `Codes::corrected`.
    #[cfg(test)]
    pub(crate) fn lane<'a>(
        &'a mut self,
        words: &'a [[u16; N]],
        lane: usize,
        calibration: Codes<'a>,
        map: Map,
    ) -> Filtered<'a, N, impl Fn(usize) -> u16 + 'a> {
        self.lane_with(move |i| words[i][lane], lane, calibration, map)
    }

    pub(in crate::picture::deposit::envelope) fn lane_with<'a, F: Fn(usize) -> u16>(
        &'a mut self,
        word: F,
        lane: usize,
        calibration: Codes<'a>,
        map: Map,
    ) -> Filtered<'a, N, F> {
        let (gain, offset) = calibration.corrected.map_or_else(
            || (f64::from(calibration.affine.0), f64::from(calibration.affine.1)),
            |c| (c.scale / 7500.0, c.offset),
        );
        let magnitude = f64::mul_add(gain.abs(), 32768.0, offset.abs());
        // Bound the f32 decode operations, the final cast and the f64 sum's roundoff.
        let error = 4.0 * (magnitude * f64::from(f32::EPSILON) + f64::from(f32::MIN_POSITIVE));
        Filtered { points: &self.points, edges: &mut self.edges, word, lane, calibration, map, gain, offset, error }
    }
}

fn reciprocal(count: usize) -> u64 {
    (1 << RECIP_BITS) / count.max(1) as u64
}

pub(crate) struct Filtered<'a, const N: usize, F> {
    points: &'a [Sum<N>],
    edges: &'a mut ColumnEdges,
    word: F,
    lane: usize,
    calibration: Codes<'a>,
    map: Map,
    gain: f64,
    offset: f64,
    error: f64,
}

impl<const N: usize, F: Fn(usize) -> u16> Filtered<'_, N, F> {
    pub(in crate::picture::deposit::envelope) fn spans(
        &mut self,
        (width, height): (usize, usize),
        cache: &mut code_rows::RowCache,
        out: &mut Spans,
    ) {
        let Some(rows) = cache.get(&self.calibration, &self.map, height) else {
            Cause::BoxcarRowTable.mark();
            return fastdraw::spans_into(width, height, self, &self.map, out);
        };
        let center = f64::from(self.calibration.volts(32768)) * f64::from(self.map.y_per_volt) + f64::from(self.map.y0);
        let gain = (self.gain * f64::from(self.map.y_per_volt) * (1u64 << 32) as f64).round() as i64;
        let center = (center * (1u64 << 32) as f64).round() as i64;
        self.edges.update(width, self.points.len(), &self.map);
        out.c0 = self.edges.ranges().c0;
        out.rows.clear();
        let error = 2.0 * self.error * f64::from(self.map.y_per_volt.abs())
            + 2.0 * f64::from(f32::EPSILON) * (f64::from(self.map.y0.abs()) + height as f64)
            + gain.unsigned_abs() as f64 / (1u64 << (32 + MEAN_BITS)) as f64;
        let margin = (error * (1u64 << 32) as f64).ceil() as u32;
        let row_at = |i| self.row_at(i, (gain, center, margin), rows);
        out.rows.extend(self.edges.ranges().first.windows(2).map(|edge| {
            let [a, b] = [edge[0], edge[1]];
            if b <= a {
                return (1, 0);
            }
            let (mut low, mut high) =
                (a..b).filter_map(row_at).fold((i32::MAX, i32::MIN), |(lo, hi), r| (lo.min(r), hi.max(r)));
            if let Some(previous) = a.checked_sub(1).and_then(row_at) {
                low = low.min(previous);
                high = high.max(previous);
            }
            fastdraw::clip_rows(low, high, height as i32)
        }));
    }

    fn row_at(&self, i: usize, (gain, center, margin): (i64, i64, u32), rows: &code_rows::RowTable) -> Option<i32> {
        let point = &self.points[i];
        let count = point.words.len() as u32;
        if count == 0 {
            return None;
        }
        let sum = point.codes[self.lane];
        if count == 1 || sum == 0 {
            return Some(rows.row(sum as u16));
        }
        if sum == u32::from(u16::MAX) * count {
            return Some(rows.row(u16::MAX));
        }
        let shift = RECIP_BITS - MEAN_BITS;
        let mean = ((u64::from(sum) * point.reciprocal + (1 << (shift - 1))) >> shift) as i64;
        let centered = mean - (32768 << MEAN_BITS);
        let row = (gain.saturating_mul(centered) >> MEAN_BITS).saturating_add(center);
        let biased = row.saturating_add(1 << 31);
        let rounded = biased >> 32;
        if (0..=32768).contains(&rounded) && (biased as u32).wrapping_add(margin) <= margin.saturating_mul(2) {
            Cause::BoxcarRounding.mark();
            return Some(code_rows::project(self.at(i), &self.map));
        }
        Some(rounded.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32)
    }
}

impl<const N: usize, F: Fn(usize) -> u16> Samples for Filtered<'_, N, F> {
    fn len(&self) -> usize {
        self.points.len()
    }

    fn at(&self, i: usize) -> f32 {
        let point = &self.points[i];
        let count = point.words.len();
        if count == 0 {
            return f32::NAN;
        }
        let mean_code = f64::from(point.codes[self.lane]) / count as f64;
        let volts = (mean_code - 32768.0) * self.gain - self.offset;
        let row = |v: f64| (v as f32 * self.map.y_per_volt + self.map.y0).round();
        let (lo, hi) = (row(volts - self.error), row(volts + self.error));
        // Moving the affine conversion after the sum may change a half-pixel tie. Only those
        // windows need the original per-word f32 conversion in the original accumulation order.
        if lo.is_finite() && lo == hi {
            return volts as f32;
        }
        Cause::BoxcarVoltageSum.mark();
        let (sum, count) = point
            .words
            .clone()
            .map(|i| (self.word)(i))
            .map(|code| self.calibration.calibrated(code, code).0)
            .filter(|y| y.is_finite())
            .fold((0.0, 0u32), |(sum, count), y| (sum + f64::from(y), count + 1));
        (sum / f64::from(count)) as f32
    }

    fn extremes(&self, a: usize, b: usize) -> (f32, f32) {
        (a..b).map(|i| self.at(i)).fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), v| (lo.min(v), hi.max(v)))
    }
}
