// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

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
    let wgt = T::weight(wgt);
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
    if v.len() == 0 || m.x_per_sample <= 0.0 {
        return 0..0;
    }
    let bw = band.len() / (h + 1);
    let c_hi = (c_lo + bw) as i64;
    let k_of = |x: f64| m.ti + (x - m.x0) / m.x_per_sample;
    let first = |c: i64| (k_of(c as f64).ceil().max(0.0) as usize).min(v.len());
    let c0 = (((-m.ti) * m.x_per_sample + m.x0).floor() as i64).max(c_lo as i64);
    let c1 = ((((v.len() as f64 - m.ti) * m.x_per_sample + m.x0).ceil()) as i64).min(c_hi);
    if c1 <= c0 {
        return 0..0;
    }
    let mut rows = crate::wave::RowBand::empty();
    let mut ka = first(c0);
    for c in c0..c1 {
        let kb = first(c + 1);
        let xc = c as usize - c_lo;
        for i in ka..kb.max(ka) {
            let x = v.at(i);
            let r = (x * m.y_per_volt + m.y0).round();
            // NaN fails both comparisons
            if r >= 0.0 && r < h as f32 {
                diff_span(band, bw, xc, r as usize, r as usize, wgt);
                rows.include(r as usize..r as usize + 1);
            }
        }
        ka = kb.max(ka);
    }
    rows.into_range()
}
