// SPDX-License-Identifier: GPL-2.0-or-later
//! Cache inverse column coordinates by geometry and reuse exact ranges for repeated trigger phases.

use crate::fastdraw::{self, Map};
use std::ops::Range;

const MAX_COORDINATE: f64 = (1u64 << 48) as f64;

#[derive(Debug, Default)]
pub(crate) struct ColumnEdges {
    grids: [Grid; 4], // Main and zoom, each with a dense and a pair grid.
    active: Option<usize>,
    next: usize,
    #[cfg(test)]
    rebuilds: (usize, usize),
}

#[derive(Debug, Default)]
pub(super) struct Ranges {
    pub(super) c0: usize,
    pub(super) first: Vec<usize>,
    pub(super) previous_pair: Vec<usize>,
}

impl ColumnEdges {
    #[cfg(test)]
    pub(crate) fn with_capacity(width: usize) -> Self {
        let mut edges = Self::default();
        edges.grids[0].ranges.first.reserve(width + 1);
        edges
    }

    #[cfg(test)]
    pub(super) fn grid_rebuilds(&self) -> usize {
        self.rebuilds.0
    }

    pub(super) fn ranges(&self) -> &Ranges {
        static EMPTY: Ranges = Ranges { c0: 0, first: Vec::new(), previous_pair: Vec::new() };
        self.active.map_or(&EMPTY, |index| &self.grids[index].ranges)
    }

    pub(crate) fn update(&mut self, width: usize, len: usize, map: &Map) {
        self.update_for(width, len, map, false);
    }

    pub(super) fn update_for(&mut self, width: usize, len: usize, map: &Map, pairs: bool) {
        if width == 0 || len == 0 || map.x_per_sample <= 0.0 {
            self.active = None;
            return;
        }
        let geometry = (width, map.x_per_sample.to_bits(), map.x0.to_bits(), pairs);
        let index = self.grids.iter().position(|grid| grid.geometry == Some(geometry)).unwrap_or_else(|| {
            let index = self.next;
            self.next = (index + 1) % self.grids.len();
            self.grids[index].prepare(geometry, map);
            #[cfg(test)]
            {
                self.rebuilds.0 += 1;
            }
            index
        });
        self.active = Some(index);
        let grid = &mut self.grids[index];
        let record = (len, map.ti.to_bits());
        if grid.record != Some(record) {
            if pairs && grid.record.is_some_and(|(_, phase)| phase != record.1) {
                grid.prepare_pair_phases(width, map);
            }
            grid.update(width, len, map, pairs);
            grid.record = Some(record);
            #[cfg(test)]
            {
                self.rebuilds.1 += 1;
            }
        }
    }
}

type Geometry = (usize, u64, u64, bool);

#[derive(Debug, Default)]
struct Grid {
    geometry: Option<Geometry>,
    record: Option<(usize, u64)>,
    ranges: Ranges,
    inverse: Vec<f64>,
    forward: Vec<Forward>,
}

impl Grid {
    fn prepare(&mut self, geometry: Geometry, map: &Map) {
        let (width, _, _, pairs) = geometry;
        self.geometry = Some(geometry);
        self.record = None;
        self.inverse.clear();
        self.inverse.extend((0..=width).map(|column| (column as f64 - map.x0) / map.x_per_sample));
        self.forward.clear();
        self.ranges.first.clear();
        self.ranges.first.reserve(width + 1);
        if pairs {
            self.ranges.previous_pair.clear();
            self.ranges.previous_pair.reserve(width + 1);
            self.forward.reserve(width + 1);
        }
    }

    fn prepare_pair_phases(&mut self, width: usize, map: &Map) {
        // Fixed trigger positions reuse the exact first range. Only changing phases need thresholds.
        if self.forward.is_empty() && self.inverse[0].abs().max(self.inverse[width].abs()) < MAX_COORDINATE {
            self.forward.extend((0..=width).map(|column| Forward::new(column, map)));
        }
    }

    fn update(&mut self, width: usize, len: usize, map: &Map, pairs: bool) {
        let columns = visible_columns(width, len, map, pairs);
        self.ranges.c0 = 0;
        self.ranges.first.clear();
        self.ranges.previous_pair.clear();
        if columns.is_empty() {
            return;
        }
        self.ranges.c0 = columns.start;
        let columns = columns.start..=columns.end;
        if !pairs {
            dense_edges(&self.inverse[columns], map.ti, len, &mut self.ranges.first);
        } else if !self.forward.is_empty() && map.ti.abs() < MAX_COORDINATE {
            let phase = Fixed::new(map.ti);
            for (&inverse, forward) in self.inverse[columns.clone()].iter().zip(&self.forward[columns]) {
                let edge = ((phase.floor + forward.cut) >> 65) as i64;
                let edge = edge + i64::from(!forward.accepts(2 * edge + 1, phase));
                self.ranges.first.push((2 * edge.max(0) as usize).min(len));
                self.ranges.previous_pair.push(previous_pair(inverse_pair(map.ti + inverse)));
            }
        } else {
            for (column, &inverse) in columns.clone().zip(&self.inverse[columns]) {
                let (start, end) = exact_pair_edge(column, len, map, inverse);
                self.ranges.first.push(start);
                self.ranges.previous_pair.push(end);
            }
        }
    }
}

fn visible_columns(width: usize, len: usize, map: &Map, pairs: bool) -> Range<usize> {
    if pairs {
        let first = (centre(0, map).floor() as usize).min(width);
        let end = (centre(len.div_ceil(2) - 1, map).floor() as i64).saturating_add(1).clamp(0, width as i64);
        first..end as usize
    } else {
        let columns = fastdraw::columns(width, len, map);
        columns.start as usize..columns.end.max(0) as usize
    }
}

fn dense_edges(inverse: &[f64], phase: f64, len: usize, first: &mut Vec<usize>) {
    #[cfg(target_arch = "aarch64")]
    let inverse = super::neon::dense_edges(inverse, phase, len, first);
    first.extend(inverse.iter().map(|&inverse| ((phase + inverse).ceil().max(0.0) as usize).min(len)));
}

/// Q64 floors preserve every bit of a binary64 with magnitude at least 2^-11.
/// A smaller operand can retain a positive remainder below the fixed-point grid.
#[derive(Clone, Copy, Debug)]
struct Fixed {
    floor: i128,
    remainder: bool,
}

const MANTISSA: u64 = (1 << 52) - 1;

impl Fixed {
    fn new(value: f64) -> Self {
        let bits = value.to_bits();
        let exp = ((bits >> 52) & 0x7ff) as u32;
        let mantissa = (bits & MANTISSA) | (u64::from(exp != 0) << 52);
        let shift = exp.max(1) as i32 - 1023 + 64 - 52;
        let (whole, remainder) = if shift >= 0 {
            (i128::from(mantissa) << shift, false)
        } else {
            let whole = mantissa.checked_shr(shift.unsigned_abs()).unwrap_or(0);
            (i128::from(whole), whole.checked_shl(shift.unsigned_abs()).unwrap_or(0) != mantissa)
        };
        let floor = if bits >> 63 == 0 { whole } else { -whole - i128::from(remainder) };
        Self { floor, remainder }
    }
}

#[derive(Debug, Default)]
struct Forward {
    cut: i128,
    inclusive: bool,
}

impl Forward {
    fn new(column: usize, map: &Map) -> Self {
        // Find the first representable difference d for which the legacy forward map
        // places a pair on or after this column. The comparison is monotone in d.
        let (mut low, mut high) = (ordered(-2.0 * MAX_COORDINATE), ordered(2.0 * MAX_COORDINATE));
        while low < high {
            let middle = low.midpoint(high) - i64::from(low.midpoint(high) == high);
            let difference = f64::from_bits(unorder(middle));
            if difference * map.x_per_sample + map.x0 >= column as f64 {
                high = middle;
            } else {
                low = middle + 1;
            }
        }
        let first = f64::from_bits(unorder(low));
        let boundary = Fixed::new(first);
        if first.abs() < 1.0 / 1024.0 {
            // Near zero, odd - ti is exact by Sterbenz's lemma. Its quantum is at least
            // 2^-53, so rounding the threshold up to Q64 does not change the comparison.
            Self { cut: boundary.floor + i128::from(boundary.remainder), inclusive: true }
        } else {
            let before = Fixed::new(first.next_down());
            Self { cut: (boundary.floor + before.floor) >> 1, inclusive: first.to_bits() & 1 == 0 }
        }
    }

    #[inline]
    fn accepts(&self, odd: i64, phase: Fixed) -> bool {
        let value = (i128::from(odd) << 64) - phase.floor;
        value > self.cut || value == self.cut && self.inclusive && !phase.remainder
    }
}

fn ordered(value: f64) -> i64 {
    ordered_bits(value.to_bits())
}

fn ordered_bits(bits: u64) -> i64 {
    let bits = if bits << 1 == 0 { 0 } else { bits as i64 };
    (bits >> 63 & i64::MAX) ^ bits
}

fn unorder(ordered: i64) -> u64 {
    ((ordered >> 63 & i64::MAX) ^ ordered) as u64
}

fn centre(pair: usize, map: &Map) -> f64 {
    ((pair * 2 + 1) as f64 - map.ti) * map.x_per_sample + map.x0
}

fn inverse_pair(edge: f64) -> i64 {
    ((edge - 1.0) / 2.0).ceil() as i64
}

fn previous_pair(inverse: i64) -> usize {
    usize::try_from(inverse.saturating_sub(1)).ok().and_then(|index| index.checked_mul(2)).unwrap_or(usize::MAX)
}

fn exact_pair_edge(column: usize, len: usize, map: &Map, inverse: f64) -> (usize, usize) {
    let inverse = inverse_pair(map.ti + inverse);
    let pairs = len.div_ceil(2);
    let mut pair = (inverse.max(0) as usize).min(pairs);
    while pair > 0 && centre(pair - 1, map) >= column as f64 {
        pair -= 1;
    }
    while pair < pairs && centre(pair, map) < column as f64 {
        pair += 1;
    }
    ((2 * pair).min(len), previous_pair(inverse))
}

#[cfg(test)]
mod tests;
