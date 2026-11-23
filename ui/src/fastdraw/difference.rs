// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::picture::deposit::count::Count;
use std::{cell::RefCell, ops::Range};

pub(crate) mod zoom;

/// The fast path collects a screen's frames as differences: a span of rows r0..=r1 with weight w is +w at r0 and -w
/// at r1 + 1 of its column (wrapping u16), whatever its height; `fold_t` sums them up once a screen into the map.
/// Written into the map directly, every pixel of a tall span was another cache line (10 us/div, 4 channels, a 1 MHz
/// sine: about 2 ms a frame on the scope). A band of `bw` columns holds `h + 1` rows of them, row by row, so the sums
/// go down a row at a time over all its columns.
#[inline(always)]
pub(crate) fn diff_span<T: crate::picture::deposit::count::Count>(
    band: &mut [T],
    bw: usize,
    x: usize,
    r0: usize,
    r1: usize,
    wgt: u16,
) {
    span(band, bw, x, r0, r1, T::weight(wgt));
}

#[inline(always)]
fn span<T: Count>(band: &mut [T], bw: usize, x: usize, r0: usize, r1: usize, wgt: T) {
    band[r0 * bw + x] = band[r0 * bw + x].wrapping_add(wgt);
    band[(r1 + 1) * bw + x] = band[(r1 + 1) * bw + x].wrapping_sub(wgt);
}

/// `add_spans` into a band of the collecting differences (`diff_span`): `band` holds the columns `c_lo ..`, `h + 1`
/// rows of them.
pub(crate) fn add_spans_t<T: crate::picture::deposit::count::Count>(
    band: &mut [T],
    h: usize,
    c_lo: usize,
    s: &Spans,
    wgt: u16,
) {
    let bw = band.len() / (h + 1);
    let c_hi = c_lo + bw;
    for c in s.c0.max(c_lo)..(s.c0 + s.rows.len()).min(c_hi) {
        let (r0, r1) = s.rows[c - s.c0];
        if r0 <= r1 {
            diff_span(band, bw, c - c_lo, r0 as usize, r1 as usize, wgt);
        }
    }
}

/// `deposit_dots` into a band of the collecting differences (`add_spans_t`): only the samples of its columns.
pub(crate) fn deposit_dots_t<T: crate::picture::deposit::count::Count, S: Samples + ?Sized>(
    band: &mut [T],
    h: usize,
    c_lo: usize,
    v: &S,
    m: &Map,
    wgt: u16,
) -> Range<usize> {
    dots(band, (h, c_lo, v.len()), m, wgt, |i| {
        let row = (v.at(i) * m.y_per_volt + m.y0).round();
        if row >= 0.0 {
            row as usize
        } else {
            usize::MAX
        }
    })
}

thread_local! {
    static DOT_ROWS: RefCell<[code_rows::RowCache; 4]> = RefCell::new(Default::default());
}

pub(crate) fn deposit_code_dots<T: Count>(
    band: &mut [T],
    h: usize,
    first: usize,
    codes: &Codes<'_>,
    map: &Map,
) -> Range<usize> {
    DOT_ROWS.with_borrow_mut(|cache| {
        if let Some(rows) = cache[codes.word % 4].get(codes, map, h) {
            dots(band, (h, first, codes.n), map, 24, |i| rows.row(codes.at_code(i)) as usize)
        } else {
            deposit_dots_t(band, h, first, codes, map, 24)
        }
    })
}

fn dots<T: Count>(
    band: &mut [T],
    (height, first, len): (usize, usize, usize),
    map: &Map,
    weight: u16,
    row_at: impl Fn(usize) -> usize,
) -> Range<usize> {
    if len == 0 || map.x_per_sample <= 0.0 {
        return 0..0;
    }
    let width = band.len() / (height + 1);
    let visible = columns(first + width, len, map);
    let start = visible.start.max(first as i64);
    let step = map.x_per_sample.recip();
    let origin = map.ti - map.x0 * step;
    const UNIT: i64 = 1 << 32;
    let fixed = |x: f64| (x * UNIT as f64).round() as i64;
    let stride = fixed(step);
    let mut edge = fixed(origin + start as f64 * step);
    let index = |column, edge: i64| {
        // At an integer edge, preserve the original ceil even when floating-point rounding put it just above.
        let fraction = edge & (UNIT - 1);
        let tolerance = width as i64 + 4;
        if fraction <= tolerance || fraction >= UNIT - tolerance {
            super::mapping::first_sample(column, len, map)
        } else {
            (((edge >> 32) + 1).max(0) as usize).min(len)
        }
    };
    let mut a = index(start, edge);
    let (mut low, mut high) = (height, 0);
    for column in start..visible.end {
        edge = edge.saturating_add(stride);
        let b = index(column + 1, edge).max(a);
        let x = column as usize - first;
        let mut put = |row, hits: u32| {
            if row < height {
                span(band, width, x, row, row, T::from_value(u32::from(weight).wrapping_mul(hits)));
                low = low.min(row);
                high = high.max(row + 1);
            }
        };
        // Consecutive dots on the same cell contribute the same count in one pair of writes.
        let (mut previous, mut hits) = (height, 0u32);
        for i in a..b {
            let row = row_at(i);
            if row != previous {
                put(previous, hits);
                previous = row;
                hits = 0;
            }
            hits += 1;
        }
        put(previous, hits);
        a = b;
    }
    low.min(high)..high
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integer_dots_agree_with_legacy_decoded_pixels_across_tile_boundaries() {
        let words: Vec<_> = (0..4096).map(|i| (i * 977) as u16).collect();
        let volt = |code: u16, (scale, offset): (f32, f32)| (f32::from(code) - 32768.0) * scale - offset;
        let codes = Codes {
            bytes: &[],
            words: Some(&words),
            stride: 2,
            word: 1,
            n: words.len() / 2,
            affine: (0.000_157, 0.23),
            corrected: None,
            volt,
        };
        let decoded: Vec<_> = (0..codes.n).map(|i| codes.at(i)).collect();
        let (width, height) = (97, 125);
        let mut expected = vec![0; width * height];
        for ti in [1024.0, 997.000_000_000_1, 966.877] {
            for x_per_sample in [0.05, 0.11, 0.5] {
                let map = Map { ti, x_per_sample, x0: 48.5, y_per_volt: -20.25, y0: 62.4 };
                expected.fill(0);
                let _ = crate::fastdraw::deposit_dots(&mut expected, width, height, &decoded, &map, 24);
                for (start, count) in [(0, 28), (28, 31), (59, 38)] {
                    let mut actual = vec![0u16; count * (height + 1)];
                    deposit_code_dots(&mut actual, height, start, &codes, &map);
                    let mut running = vec![0u16; count];
                    for row in 0..height {
                        for col in 0..count {
                            running[col] = running[col].wrapping_add(actual[row * count + col]);
                            assert_eq!(running[col], expected[row * width + start + col], "row={row}, tile={start}");
                        }
                    }
                }
            }
        }
    }
}

/// row-major `map` (`w` x `h`) and clears them: down each band a row at a time, its columns side by side (vector
/// instructions), rows with nothing running pass the map by.
pub fn fold_t(map: &mut [u16], acc: &mut [u16], w: usize, h: usize, band_cols: usize, rows: Range<usize>) {
    let mut run = vec![0u16; band_cols.min(w)];
    if band_cols == 0 {
        return;
    }
    for (b, band) in acc.chunks_mut(band_cols * (h + 1)).enumerate() {
        let bw = band.len() / (h + 1);
        crate::picture::deposit::kernel::fold_band(
            map,
            band,
            &mut run[..bw],
            (w, h, b * band_cols),
            rows.clone(),
            true,
        );
    }
}

/// The first fold: differences column by column (`h + 1` a column), summed down the columns in tiles (the reference
/// of the test, and the bench's baseline).
#[cfg(test)]
fn fold_t_cols(map: &mut [u16], acc: &mut [u16], w: usize, h: usize, rows: Range<usize>) {
    const T: usize = 32;
    let hp = h + 1;
    let (r_lo, r_end) = (rows.start.min(h), (rows.end.min(h) + 1).min(hp));
    if r_lo >= r_end {
        return;
    }
    let mut run = vec![0u16; w];
    for ra in (r_lo..r_end).step_by(T) {
        let rb = (ra + T).min(r_end);
        for ca in (0..w).step_by(T) {
            for c in ca..(ca + T).min(w) {
                let col = &mut acc[c * hp + ra..c * hp + rb];
                let mut v = run[c];
                if v == 0 && col.iter().fold(0u16, |a, &x| a | x) == 0 {
                    continue;
                }
                for (j, x) in col.iter_mut().enumerate() {
                    v = v.wrapping_add(*x);
                    *x = 0;
                    let r = ra + j;
                    if v != 0 && r < h {
                        let i = r * w + c;
                        map[i] = map[i].saturating_add(v);
                    }
                }
                run[c] = v;
            }
        }
    }
}

#[cfg(test)]
mod fold_tests;

#[cfg(test)]
pub fn add_spans(band: &mut [u16], w: usize, r_lo: usize, s: &Spans, wgt: u16) {
    crate::picture::deposit::kernel::paint_spans(band, w, r_lo, s, wgt);
}
