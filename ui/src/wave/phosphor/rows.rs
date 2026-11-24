// SPDX-License-Identifier: GPL-2.0-or-later

use std::ops::Range;

/// The hull of nonempty row ranges, with a canonical empty range.
pub(crate) struct RowBand(Range<usize>);

impl RowBand {
    pub(crate) fn empty() -> Self {
        Self(0..0)
    }

    #[inline]
    pub(crate) fn include(&mut self, rows: Range<usize>) {
        if rows.is_empty() {
            return;
        }
        self.0 = if self.0.is_empty() { rows } else { self.0.start.min(rows.start)..self.0.end.max(rows.end) };
    }

    pub(crate) fn into_range(self) -> Range<usize> {
        self.0
    }
}

/// Per-channel coverage since the last clear, independent of publication and upload revisions.
pub(super) struct Touched {
    band: RowBand,
    rows: Vec<bool>,
}

impl Touched {
    pub(super) fn new(height: usize) -> Self {
        Self { band: RowBand::empty(), rows: vec![false; height] }
    }

    pub(super) fn mark(&mut self, rows: Range<usize>) {
        let h = self.rows.len();
        let rows = rows.start.min(h)..rows.end.min(h);
        if !rows.is_empty() {
            self.rows[rows.clone()].fill(true);
            self.band.include(rows);
        }
    }

    pub(super) fn clear<T: crate::picture::deposit::count::Count>(
        &mut self,
        map: &mut [T],
        width: usize,
    ) -> Range<usize> {
        let rows = std::mem::replace(&mut self.band, RowBand::empty()).into_range();
        let zero = T::default();
        let mut changed = RowBand::empty();
        for y in rows {
            if !std::mem::take(&mut self.rows[y]) {
                continue;
            }
            let row = &mut map[y * width..(y + 1) * width];
            if row.iter().any(|&value| value != zero) {
                row.fill(zero);
                changed.include(y..y + 1);
            }
        }
        changed.into_range()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clear_visits_written_rows_and_resets_the_coverage() {
        let mut touched = Touched::new(6);
        let mut map = [1u32; 12];
        touched.mark(1..2);
        touched.mark(4..5);
        assert_eq!(touched.clear(&mut map, 2), 1..5);
        assert_eq!(map, [1, 1, 0, 0, 1, 1, 1, 1, 0, 0, 1, 1]);
        assert_eq!(touched.clear(&mut map, 2), 0..0);
    }

    #[test]
    fn hull_ignores_empty_ranges_and_includes_disjoint_rows() {
        let mut band = RowBand::empty();
        band.include(8..8);
        assert_eq!(band.into_range(), 0..0);
        let mut band = RowBand::empty();
        for rows in [12..15, 0..0, 3..5, 7..9, 2..18] {
            band.include(rows);
        }
        assert_eq!(band.into_range(), 2..18);
    }
}
