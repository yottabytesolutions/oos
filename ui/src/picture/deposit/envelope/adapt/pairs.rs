// SPDX-License-Identifier: GPL-2.0-or-later
//! Live and deep-memory pair envelopes. Both words belong to the column of the pair centre.

use super::super::{ColumnEdges, Correction, Envelopes, Extrema};
use crate::drawpipe::stage::fixture_stage;
use crate::fastdraw::{self, code_rows::RowTable, Codes, Map, Samples, Spans};
use std::ops::Range;

/// Lines join adjacent pairs and fill sparse gaps. Dots show only the occupied columns.
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
            (dots, map.x_per_sample),
            spans,
            |a, b| samples.extremes(a, b),
            |low, high| fastdraw::mapped_rows((low, high), None, map, height),
        )
    );
}

pub(in crate::picture::deposit::envelope) fn code_spans_into(
    codes: &Codes<'_>,
    edges: &ColumnEdges,
    (width, x_per_sample): (usize, f64),
    dots: bool,
    rows: &RowTable,
    spans: &mut Spans,
) {
    fixture_stage!(
        FusedReduceMap,
        pair_spans(
            edges,
            (width, codes.n),
            (dots, x_per_sample),
            spans,
            |a, b| codes.code_extremes(a, b),
            |low, high| rows.span(low, high),
        )
    );
}

fn pair_spans<T: Copy + PartialOrd>(
    edges: &ColumnEdges,
    (width, len): (usize, usize),
    (dots, x_per_sample): (bool, f64),
    spans: &mut Spans,
    extremes: impl Fn(usize, usize) -> (T, T),
    rows: impl Fn(T, T) -> (i16, i16),
) {
    spans.c0 = 0;
    spans.rows.clear();
    spans.rows.resize(width, (1, 0));
    let ranges = edges.ranges();
    let bridge_gaps = !dots && x_per_sample > 0.5;
    let mut gaps = GapBridge(None);
    for (offset, &[first, end]) in ranges.first.array_windows().enumerate() {
        let column = ranges.c0 + offset;
        if first == end {
            if bridge_gaps {
                spans.rows[column] = gaps
                    .span(first, len, |left, right| {
                        let (a, b) = extremes(left, right);
                        let (c, d) = extremes(right, (right + 2).min(len));
                        if a <= b && c <= d {
                            let (low, high) = connecting_span((a, b), (c, d));
                            rows(low, high)
                        } else {
                            (1, 0)
                        }
                    })
                    .unwrap_or((1, 0));
            }
            continue;
        }
        let (mut low, mut high) = extremes(first, end);
        if !dots && low <= high {
            let start = ranges.previous_pair[offset];
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
        spans.rows[column] = rows(low, high);
    }
}

pub(in crate::picture::deposit::envelope) fn join_codes<const N: usize>(
    words: &[[u16; N]],
    correction: &Correction<N>,
    edges: &ColumnEdges,
    (dots, x_per_sample): (bool, f64),
    out: &mut Envelopes<N>,
) {
    if dots {
        return;
    }
    let len = words.len();
    let bridge_gaps = x_per_sample > 0.5;
    let mut gaps = GapBridge(None);
    let ranges = edges.ranges();
    for ((column, &previous), &[first, end]) in
        out.columns.iter_mut().zip(&ranges.previous_pair).zip(ranges.first.array_windows())
    {
        if first == end {
            if bridge_gaps {
                *column = gaps
                    .span(first, len, |left, right| {
                        let left = code_pair(words, correction, left);
                        let right = code_pair(words, correction, right);
                        let mut bridge = left;
                        for lane in 0..N {
                            (bridge.low[lane], bridge.high[lane]) =
                                connecting_span((left.low[lane], left.high[lane]), (right.low[lane], right.high[lane]));
                        }
                        bridge
                    })
                    .unwrap_or(Extrema::EMPTY);
            }
            continue;
        }
        if previous < len {
            let pair = code_pair(words, correction, previous);
            for lane in 0..N {
                column.low[lane] = column.low[lane].min(pair.high[lane]);
                column.high[lane] = column.high[lane].max(pair.low[lane]);
            }
        }
    }
}

#[inline]
fn code_pair<const N: usize>(words: &[[u16; N]], correction: &Correction<N>, first: usize) -> Extrema<N> {
    let last = (first + 1).min(words.len() - 1);
    let (a, b) = (words[first], words[last]);
    let (a, b) = if correction.active { (correction.row(a, first), correction.row(b, last)) } else { (a, b) };
    Extrema {
        low: std::array::from_fn(|lane| a[lane].min(b[lane])),
        high: std::array::from_fn(|lane| a[lane].max(b[lane])),
    }
}

/// Consecutive empty columns have the same next pair. Read their two endpoints once for the whole gap.
struct GapBridge<T>(Option<(usize, T)>);

impl<T: Copy> GapBridge<T> {
    #[inline]
    fn span(&mut self, right: usize, len: usize, connect: impl FnOnce(usize, usize) -> T) -> Option<T> {
        if !(2..len).contains(&right) {
            return None;
        }
        if let Some((_, span)) = self.0.filter(|&(cached, _)| cached == right) {
            return Some(span);
        }
        let span = connect(right - 2, right);
        self.0 = Some((right, span));
        Some(span)
    }
}

/// Bridge the nearest ends of disjoint pairs or cover the overlap of intersecting pairs.
#[inline]
fn connecting_span<T: Copy + PartialOrd>((a, b): (T, T), (c, d): (T, T)) -> (T, T) {
    let lower = if a < c { c } else { a };
    let upper = if b < d { b } else { d };
    if lower <= upper {
        (lower, upper)
    } else {
        (upper, lower)
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
        let Some(range) = screen_pair_range(2 * j, (sample_dt, per), (dt, first), ti, values.len()) else {
            continue;
        };
        let span = &values[range];
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

/// Sample indices for one overlapping screen window, starting at the even output point.
#[inline]
pub(crate) fn screen_pair_range(
    point: usize,
    (sample_dt, per): (f64, f64),
    (dt, first): (f64, f64),
    ti: f64,
    len: usize,
) -> Option<Range<usize>> {
    let center = ti + (first + point as f64 * dt + 0.5 * dt) / sample_dt;
    if center < -per || center >= len as f64 + per {
        return None;
    }
    let start = (center - per).max(0.0) as usize;
    let end = ((center + per).ceil() as usize).min(len);
    Some(start.min(end)..end)
}
