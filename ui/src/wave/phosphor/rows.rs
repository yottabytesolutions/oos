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

#[cfg(test)]
mod tests {
    use super::*;

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
