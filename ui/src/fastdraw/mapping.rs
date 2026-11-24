// SPDX-License-Identifier: GPL-2.0-or-later
//! Exact scalar column and row mapping shared by independent and interleaved sample walks.
use super::{code_rows::RowCache, Codes, Map, Samples, Spans};
use std::ops::Range;
#[inline]
pub(crate) fn columns(width: usize, len: usize, m: &Map) -> Range<i64> {
    let first = (((-m.ti) * m.x_per_sample + m.x0).floor() as i64).max(0);
    let last = ((((len as f64 - m.ti) * m.x_per_sample + m.x0).ceil()) as i64).min(width as i64);
    first..last
}
#[inline]
pub(crate) fn first_sample(column: i64, len: usize, m: &Map) -> usize {
    ((m.ti + (column as f64 - m.x0) / m.x_per_sample).ceil().max(0.0) as usize).min(len)
}
#[inline]
pub(crate) fn mapped_rows((mut mn, mut mx): (f32, f32), previous: Option<f32>, m: &Map, height: usize) -> (i16, i16) {
    if let Some(previous) = previous {
        if previous.is_finite() {
            mn = mn.min(previous);
            mx = mx.max(previous);
        }
    }
    if mn.partial_cmp(&mx).is_none_or(|o| o.is_gt()) {
        return (1, 0);
    }
    let (ya, yb) = (project(mn, m), project(mx, m));
    clip_rows(ya.min(yb).round() as i32, ya.max(yb).round() as i32, height as i32)
}

#[inline]
pub(super) fn project(volts: f32, map: &Map) -> f32 {
    volts * map.y_per_volt + map.y0
}

#[inline]
pub(crate) fn clip_rows(low: i32, high: i32, height: i32) -> (i16, i16) {
    let r0 = low.max(0);
    let r1 = high.min(height - 1);
    if r0 <= r1 {
        (r0 as i16, r1 as i16)
    } else {
        (1, 0)
    }
}

pub(crate) fn spans_into(w: usize, h: usize, samples: &(impl Samples + ?Sized), map: &Map, out: &mut Spans) {
    spans_with(w, h, samples.len(), map, out, |a, b| {
        mapped_rows(samples.extremes(a, b), a.checked_sub(1).map(|i| samples.at(i)), map, h)
    });
}

#[cfg(test)]
pub(crate) fn spans_mapped(
    w: usize,
    h: usize,
    len: usize,
    map: &Map,
    out: &mut Spans,
    mut samples: impl FnMut(usize, usize) -> ((f32, f32), Option<f32>),
) {
    spans_with(w, h, len, map, out, |a, b| {
        let (extrema, previous) = samples(a, b);
        mapped_rows(extrema, previous, map, h)
    });
}

pub(crate) fn code_spans_into(w: usize, h: usize, codes: &Codes<'_>, map: &Map, cache: &mut RowCache, out: &mut Spans) {
    let Some(rows) = cache.get(codes, map, h) else { return spans_into(w, h, codes, map, out) };
    spans_with(w, h, codes.n, map, out, |a, b| {
        let (low, high) = codes.code_extremes(a, b);
        let previous = a.checked_sub(1).map_or(low, |i| codes.at_code(i));
        rows.span(low.min(previous), high.max(previous))
    });
}

pub(crate) fn spans_with(
    w: usize,
    h: usize,
    len: usize,
    m: &Map,
    out: &mut Spans,
    mut rows: impl FnMut(usize, usize) -> (i16, i16),
) {
    out.c0 = 0;
    out.rows.clear();
    if len == 0 || m.x_per_sample <= 0.0 || w == 0 || h == 0 {
        return;
    }
    let columns = columns(w, len, m);
    if columns.is_empty() {
        return;
    }
    out.c0 = columns.start as usize;
    out.rows.reserve((columns.end - columns.start) as usize);
    let mut ka = first_sample(columns.start, len, m);
    for c in columns {
        let kb = first_sample(c + 1, len, m);
        if kb <= ka {
            out.rows.push((1, 0));
            continue;
        }
        out.rows.push(rows(ka, kb));
        ka = kb;
    }
}
