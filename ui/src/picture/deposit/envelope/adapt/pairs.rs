// SPDX-License-Identifier: GPL-2.0-or-later
//! Live and deep-memory pair envelopes. Both words belong to the column of the pair centre.

use super::super::{ColumnEdges, Correction, Envelopes};
use crate::drawpipe::stage::fixture_stage;
use crate::fastdraw::{self, code_rows::RowTable, Codes, Map, Samples, Spans};
use std::ops::Range;

/// The legacy line path joins to the nearest end of a disjoint preceding pair. Dots omit this join.
pub(crate) fn spans_into(
    samples: &(impl Samples + ?Sized),
    edges: &mut ColumnEdges,
    map: &Map,
    (width, height): (usize, usize),
    dots: bool,
    spans: &mut Spans,
) {
    fixture_stage!(ColumnEdges, edges.update_for(width, samples.len(), map, true));
    spans_with_edges(samples, edges, map, (width, height), dots, spans);
}

pub(in crate::picture::deposit::envelope) fn spans_with_edges(
    samples: &(impl Samples + ?Sized),
    edges: &ColumnEdges,
    map: &Map,
    (width, height): (usize, usize),
    dots: bool,
    spans: &mut Spans,
) {
    fixture_stage!(
        FusedReduceMap,
        pair_spans(
            edges,
            (width, samples.len()),
            dots,
            spans,
            |a, b| samples.extremes(a, b),
            |low, high| fastdraw::mapped_rows((low, high), None, map, height),
        )
    );
}

pub(in crate::picture::deposit::envelope) fn code_spans_into(
    codes: &Codes<'_>,
    edges: &ColumnEdges,
    width: usize,
    dots: bool,
    rows: &RowTable,
    spans: &mut Spans,
) {
    fixture_stage!(
        FusedReduceMap,
        pair_spans(
            edges,
            (width, codes.n),
            dots,
            spans,
            |a, b| codes.code_extremes(a, b),
            |low, high| rows.span(low, high),
        )
    );
}

fn pair_spans<T: Copy + PartialOrd>(
    edges: &ColumnEdges,
    (width, len): (usize, usize),
    dots: bool,
    spans: &mut Spans,
    extremes: impl Fn(usize, usize) -> (T, T),
    rows: impl Fn(T, T) -> (i16, i16),
) {
    spans.c0 = 0;
    spans.rows.clear();
    spans.rows.resize(width, (1, 0));
    for (offset, &[first, end]) in edges.ranges().first.array_windows().enumerate() {
        if first == end {
            continue;
        }
        let (mut low, mut high) = extremes(first, end);
        if !dots && low <= high {
            let start = edges.ranges().previous_pair[offset];
            if start < len {
                let (a, b) = extremes(start, (start + 2).min(len));
                if a <= b {
                    if b < low {
                        low = b;
                    }
                    if a > high {
                        high = a;
                    }
                }
            }
        }
        spans.rows[edges.ranges().c0 + offset] = rows(low, high);
    }
}

pub(in crate::picture::deposit::envelope) fn join_codes<const N: usize>(
    words: &[[u16; N]],
    correction: &Correction<N>,
    edges: &ColumnEdges,
    dots: bool,
    out: &mut Envelopes<N>,
) {
    for ((column, &previous), &[first, end]) in
        out.columns.iter_mut().zip(&edges.ranges().previous_pair).zip(edges.ranges().first.array_windows())
    {
        if dots || first == end || previous >= words.len() {
            continue;
        }
        let last = (previous + 1).min(words.len() - 1);
        let (a, b) = (words[previous], words[last]);
        let (a, b) = if correction.active { (correction.row(a, previous), correction.row(b, last)) } else { (a, b) };
        for lane in 0..N {
            column.low[lane] = column.low[lane].min(a[lane].max(b[lane]));
            column.high[lane] = column.high[lane].max(a[lane].min(b[lane]));
        }
    }
}

/// Reduce the overlapping legacy screen windows to a min/max pair per screen point pair.
/// Keep both endpoints for the rasterizer, which deposits each segment with its own weight.
pub(crate) fn screen_envelope(
    values: &[f32],
    (sample_dt, per): (f64, f64),
    (n, dt, first): (usize, f64, f64),
    ti: f64,
    out: &mut Vec<f32>,
) {
    fixture_stage!(
        Resample,
        screen_envelope_columns(values, (sample_dt, per), (n, dt, first), ti, 0..n.div_ceil(2), out)
    );
}

pub(crate) fn screen_envelope_columns(
    values: &[f32],
    (sample_dt, per): (f64, f64),
    (n, dt, first): (usize, f64, f64),
    ti: f64,
    pairs: Range<usize>,
    out: &mut Vec<f32>,
) {
    // Two output points share the extremes of one window so narrow spikes stay visible.
    out.clear();
    out.resize((2 * pairs.end).min(n) - 2 * pairs.start, f32::NAN);
    for (j, pair) in pairs.zip(out.chunks_mut(2)) {
        let xm = ti + (first + (2 * j) as f64 * dt + 0.5 * dt) / sample_dt;
        if xm < -per || xm >= values.len() as f64 + per {
            continue;
        }
        let a = (xm - per).max(0.0) as usize;
        let b = ((xm + per).ceil() as usize).min(values.len());
        let span = &values[a.min(b)..b];
        let (mut lo, mut hi) = crate::fastdraw::min_max(span);
        if lo == f32::NEG_INFINITY || hi == f32::INFINITY {
            (lo, hi) = span
                .iter()
                .filter(|y| y.is_finite())
                .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), &y| (lo.min(y), hi.max(y)));
        }
        if lo <= hi {
            pair[0] = lo;
            if let Some(high) = pair.get_mut(1) {
                *high = hi;
            }
        }
    }
}
