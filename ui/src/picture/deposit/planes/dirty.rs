// SPDX-License-Identifier: GPL-2.0-or-later

use crate::fastdraw::{diff_span, Spans};
use crate::picture::deposit::count::Count;
use std::ops::Range;

const TILE_CELLS: usize = 128;
const BITS: usize = u64::BITS as usize;

pub(super) struct Dirty {
    tiles: Vec<u64>,
    coarse_rows: Range<usize>,
}

impl Dirty {
    pub(super) fn new(cells: usize) -> Self {
        let groups = cells.div_ceil(TILE_CELLS * BITS);
        Self { tiles: vec![0; groups], coarse_rows: 0..0 }
    }

    pub(super) fn include_rows(&mut self, rows: Range<usize>) {
        if !rows.is_empty() {
            let previous = &self.coarse_rows;
            self.coarse_rows =
                if previous.is_empty() { rows } else { previous.start.min(rows.start)..previous.end.max(rows.end) };
        }
    }

    pub(super) fn merge(&mut self, other: &Self) {
        self.include_rows(other.coarse_rows.clone());
        for (ours, theirs) in self.tiles.iter_mut().zip(&other.tiles) {
            *ours |= *theirs;
        }
    }

    pub(super) fn reset(&mut self) {
        self.tiles.fill(0);
        self.coarse_rows = 0..0;
    }

    pub(super) fn clear<T: Count>(&mut self, diff: &mut [T], width: usize) {
        let rows = std::mem::take(&mut self.coarse_rows);
        if !rows.is_empty() {
            let end = ((rows.end + 1) * width).min(diff.len());
            diff[(rows.start * width).min(end)..end].fill(T::default());
        }
        for (group, group_tiles) in self.tiles.iter_mut().enumerate() {
            let mut bits = std::mem::take(group_tiles);
            while bits != 0 {
                let start = bits.trailing_zeros();
                let end = start + (bits >> start).trailing_ones();
                let offset = group * BITS;
                let begin = (offset + start as usize) * TILE_CELLS;
                let end_cell = ((offset + end as usize) * TILE_CELLS).min(diff.len());
                diff[begin..end_cell].fill(T::default());
                bits &= u64::MAX.checked_shl(end).unwrap_or(0);
            }
        }
    }

    pub(super) fn paint<T: Count>(
        &mut self,
        diff: &mut [T],
        (first, width): (usize, usize),
        spans: &Spans,
    ) -> Range<usize> {
        let start = spans.c0.max(first);
        let end = (spans.c0 + spans.rows.len()).min(first + width);
        if start >= end {
            return 0..0;
        }
        let mut top = usize::MAX;
        let mut bottom = 0;
        let rows = &spans.rows[start - spans.c0..end - spans.c0];
        for (column, &(low, high)) in (start - first..).zip(rows) {
            if low <= high {
                let (low, high) = (low as usize, high as usize);
                diff_span(diff, width, column, low, high, 24);
                self.mark(low * width + column);
                self.mark((high + 1) * width + column);
                top = top.min(low);
                bottom = bottom.max(high + 1);
            }
        }
        top.min(bottom)..bottom
    }

    #[inline]
    fn mark(&mut self, cell: usize) {
        let tile = cell / TILE_CELLS;
        let group = tile / BITS;
        self.tiles[group] |= 1 << (tile % BITS);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sparse_and_merged_spans_clear_only_written_tiles_including_the_sentinel() {
        for width in [1, 405] {
            let len = width * 449;
            let mut diff = vec![0u32; len];
            let (mut dirty, mut other) = (Dirty::new(len), Dirty::new(len));
            let spans = Spans { c0: 4, rows: vec![(2, 30), (414, 447)] };
            dirty.paint(&mut diff, (4, width), &spans);
            other.paint(&mut diff, (5, width), &spans);
            dirty.merge(&other);
            assert!(diff.iter().any(|&cell| cell != 0));
            let untouched = 220 * width;
            diff[untouched] = 17;
            dirty.clear(&mut diff, width);
            assert_eq!(diff[untouched], 17);
            diff[untouched] = 0;
            assert!(diff.iter().all(|&cell| cell == 0));
            assert!(dirty.tiles.iter().all(|&bits| bits == 0));
        }
    }
}
