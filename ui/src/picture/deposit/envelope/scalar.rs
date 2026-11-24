// SPDX-License-Identifier: GPL-2.0-or-later

use super::{Correction, Extrema, Range};

#[inline]
pub(super) fn reduce<const N: usize>(
    words: &[[u16; N]],
    correction: &Correction<N>,
    range: Range<usize>,
) -> Extrema<N> {
    let end = range.end.min(words.len());
    let range = range.start.min(end)..end;
    let mut extrema = Extrema::EMPTY;
    for (index, &row) in (range.start..).zip(&words[range]) {
        let row = if correction.active { correction.row(row, index) } else { row };
        for (lane, code) in row.into_iter().enumerate() {
            extrema.low[lane] = extrema.low[lane].min(code);
            extrema.high[lane] = extrema.high[lane].max(code);
        }
    }
    extrema
}
