// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::drawpipe::metrics::fallback::Cause;
use crate::drawpipe::stage::fixture_stage;
use crate::drawpipe::CodeLayout;
use crate::fastdraw::Samples;
use crate::picture::deposit::kernel::{raw_map, Projection, SamplesRef};
use std::ops::Range;

#[derive(Clone, Copy)]
pub(crate) enum Layout {
    One(LaneMap<1>),
    Two(LaneMap<2>),
    Four(LaneMap<4>),
}

impl Layout {
    pub(crate) fn new(layout: &CodeLayout) -> Option<Self> {
        let finite = layout
            .word
            .iter()
            .zip(layout.affine)
            .all(|(lane, (scale, offset))| lane.is_none() || scale.is_finite() && offset.is_finite());
        if !finite
            || !layout.corrected.is_none_or(|cal| {
                cal.into_iter()
                    .zip(layout.word)
                    .all(|(c, lane)| lane.is_none() || c.volts(0).is_finite() && c.volts(u16::MAX).is_finite())
            })
        {
            return None;
        }
        Self::from_words(layout.stride, layout.word)
    }

    fn from_words(stride: usize, words: [Option<usize>; NCH]) -> Option<Self> {
        let mask: u32 = words.iter().enumerate().fold(0, |mask, (c, word)| mask | (u32::from(word.is_some()) << c));
        match (stride, mask.count_ones()) {
            (1, 1) => LaneMap::new(words, mask).map(Self::One),
            (2, 2) => LaneMap::new(words, mask).map(Self::Two),
            (4, 3 | 4) => LaneMap::new(words, mask).map(Self::Four),
            _ => None,
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct CodePlacement {
    layout: Layout,
    size: (usize, usize),
    maps: [Option<Map>; NCH],
}

impl CodePlacement {
    pub(crate) fn new(layout: Layout, projection: Projection, trigger_at: f64) -> Self {
        Self {
            layout,
            size: (projection.cols, projection.rows),
            maps: std::array::from_fn(|c| raw_map(&projection, c, trigger_at)),
        }
    }

    pub(crate) fn update_edges(self, samples: &[SamplesRef<'_>; NCH], edges: &mut ColumnEdges) -> usize {
        let inputs = code_inputs(samples, &self.maps);
        let Some((first, map)) = inputs.iter().flatten().next() else { return 0 };
        let Some(words) = record_words(&inputs, first) else { return 0 };
        let len = words.len() / first.stride;
        fixture_stage!(ColumnEdges, edges.update(self.size.0, len, map));
        edges.ranges().first.len().saturating_sub(1)
    }
}

enum Edges<'a> {
    Record(&'a mut ColumnEdges),
    Pairs(&'a mut ColumnEdges, bool),
    Columns(usize, &'a [usize]),
}

#[derive(Default)]
pub(crate) struct Scratch {
    edges: ColumnEdges,
    rows: [RowCache; NCH],
    one: Envelopes<1>,
    two: Envelopes<2>,
    four: Envelopes<4>,
    boxcar: adapt::boxcar::Boxcar<1>,
    filtered: Vec<f32>,
}

impl<const N: usize> Default for Envelopes<N> {
    fn default() -> Self {
        Self::with_capacity(0)
    }
}

impl Scratch {
    #[cfg(test)]
    pub(crate) fn cache_rebuilds(&self) -> (usize, usize) {
        (self.rows.iter().map(RowCache::rebuilds).sum(), self.edges.grid_rebuilds())
    }

    pub(crate) fn place(
        &mut self,
        layout: Layout,
        samples: &[SamplesRef<'_>; NCH],
        proj: &Projection,
        ti: f64,
        spans: &mut [Option<Spans>; NCH],
    ) {
        self.place_with(CodePlacement::new(layout, *proj, ti), samples, spans, None);
    }

    pub(crate) fn place_columns(
        &mut self,
        placement: CodePlacement,
        samples: &[SamplesRef<'_>; NCH],
        edges: &ColumnEdges,
        columns: Range<usize>,
        spans: &mut [Option<Spans>; NCH],
    ) {
        let edges = (edges.ranges().c0 + columns.start, &edges.ranges().first[columns.start..=columns.end]);
        self.place_with(placement, samples, spans, Some(edges));
    }

    fn place_with(
        &mut self,
        placement: CodePlacement,
        samples: &[SamplesRef<'_>; NCH],
        spans: &mut [Option<Spans>; NCH],
        columns: Option<(usize, &[usize])>,
    ) {
        let CodePlacement { layout, size, maps } = placement;
        let inputs = code_inputs(samples, &maps);
        let edges = columns.map_or(Edges::Record(&mut self.edges), |(c0, first)| Edges::Columns(c0, first));
        match layout {
            Layout::One(lanes) => place(lanes, inputs, size, spans, (edges, &mut self.one, &mut self.rows)),
            Layout::Two(lanes) => place(lanes, inputs, size, spans, (edges, &mut self.two, &mut self.rows)),
            Layout::Four(lanes) => place(lanes, inputs, size, spans, (edges, &mut self.four, &mut self.rows)),
        }
    }

    pub(crate) fn code_spans(
        &mut self,
        c: usize,
        size: (usize, usize),
        codes: &Codes<'_>,
        map: &Map,
        span: &mut Spans,
    ) {
        fixture_stage!(
            FusedReduceMap,
            c,
            fastdraw::code_spans_into(size.0, size.1, codes, map, &mut self.rows[c], span)
        );
    }

    pub(crate) fn pairs(
        &mut self,
        (input, layout): (&[SamplesRef<'_>; NCH], Option<Layout>),
        proj: &Projection,
        ti: f64,
        spans: &mut [Option<Spans>; NCH],
    ) {
        let maps = std::array::from_fn(|c| raw_map(proj, c, ti));
        let codes = code_inputs(input, &maps);
        let size = (proj.cols, proj.rows);
        let edges = Edges::Pairs(&mut self.edges, proj.dots);
        if let Some(layout) = layout {
            for (span, code) in spans.iter_mut().zip(&codes) {
                if code.is_none() {
                    *span = None;
                }
            }
            match layout {
                Layout::One(lanes) => place(lanes, codes, size, spans, (edges, &mut self.one, &mut self.rows)),
                Layout::Two(lanes) => place(lanes, codes, size, spans, (edges, &mut self.two, &mut self.rows)),
                Layout::Four(lanes) => place(lanes, codes, size, spans, (edges, &mut self.four, &mut self.rows)),
            }
            return;
        }
        let mut edge_len = 0;
        for (c, (sample, span)) in input.iter().zip(spans).enumerate() {
            let Some(map) = maps[c].filter(|_| sample.len() > 0) else {
                *span = None;
                continue;
            };
            if sample.len() != edge_len {
                edge_len = sample.len();
                fixture_stage!(ColumnEdges, self.edges.update_for(proj.cols, edge_len, &map, true));
            }
            let out = span.get_or_insert_with(Spans::default);
            if let SamplesRef::Codes(codes) = sample {
                if let Some(rows) = self.rows[c].get(codes, &map, proj.rows) {
                    adapt::pairs::code_spans_into(
                        codes,
                        &self.edges,
                        (proj.cols, map.x_per_sample),
                        proj.dots,
                        rows,
                        out,
                    );
                    continue;
                }
            }
            match sample {
                SamplesRef::Codes(_) => Cause::PairRowTable,
                SamplesRef::Volts(_) => Cause::DecodedPairs,
            }
            .mark();
            adapt::pairs::spans_with_edges(sample, &self.edges, &map, size, proj.dots, out);
        }
    }

    pub(crate) fn boxcar(
        &mut self,
        input: &[SamplesRef<'_>; NCH],
        proj: &Projection,
        (ti, sample_dt): (f64, f64),
        spans: &mut [Option<Spans>; NCH],
    ) {
        use crate::model::{AcqMode, HDIV};
        let window = proj.time_per_div * HDIV as f64;
        let n = crate::sparse::POINTS;
        let dt = window / n as f64;
        let first = proj.delay - window / 2.0;
        let grid = adapt::boxcar::ScreenPoints::new(first, dt, sample_dt, ti, n);
        let view = Projection { x_per_sample: proj.cols as f64 / n as f64, ..*proj };
        for (c, (sample, span)) in input.iter().zip(spans).enumerate() {
            let Some(map) = raw_map(&view, c, -first / dt).filter(|_| sample.len() >= 2) else {
                *span = None;
                continue;
            };
            let out = span.get_or_insert_with(Spans::default);
            match sample {
                SamplesRef::Codes(codes) => {
                    let word = |i| codes.at_code(i);
                    if fixture_stage!(Resample, c, self.boxcar.filter_with(codes.n, &grid, |i| [word(i)])) {
                        let mut filtered = self.boxcar.lane_with(word, 0, *codes, map);
                        fixture_stage!(
                            FusedReduceMap,
                            c,
                            filtered.spans((view.cols, view.rows), &mut self.rows[c], out)
                        );
                    } else {
                        out.rows.clear();
                    }
                }
                SamplesRef::Volts(values) => {
                    Cause::BoxcarDecoded.mark();
                    crate::board::record::resample_channel(
                        values,
                        (sample_dt, false),
                        AcqMode::HiRes,
                        (n, dt, first),
                        ti,
                        &mut self.filtered,
                    );
                    fixture_stage!(
                        FusedReduceMap,
                        c,
                        fastdraw::spans_into(view.cols, view.rows, self.filtered.as_slice(), &map, out)
                    );
                }
            }
        }
    }
}

fn code_inputs<'a>(samples: &[SamplesRef<'a>; NCH], maps: &[Option<Map>; NCH]) -> [Option<(Codes<'a>, Map)>; NCH] {
    std::array::from_fn(|c| match samples[c] {
        SamplesRef::Codes(codes) if codes.n > 0 => maps[c].map(|map| (codes, map)),
        _ => None,
    })
}

fn record_words<'a>(inputs: &[Option<(Codes<'a>, Map)>; NCH], first: &Codes<'a>) -> Option<&'a [u16]> {
    first.words.filter(|words| {
        let len = words.len() / first.stride;
        inputs.iter().flatten().all(|(codes, _)| codes.n == len)
    })
}

fn place<const N: usize>(
    lanes: LaneMap<N>,
    inputs: [Option<(Codes<'_>, Map)>; NCH],
    (width, height): (usize, usize),
    spans: &mut [Option<Spans>; NCH],
    (mut edges, out, rows): (Edges<'_>, &mut Envelopes<N>, &mut [RowCache; NCH]),
) {
    let Some((first, map)) = inputs.iter().flatten().next() else { return };
    let Some(words) = record_words(&inputs, first) else {
        // A short final group or unaligned read-out can still contain complete samples of a channel.
        for ((span, cache), input) in spans.iter_mut().zip(rows).zip(inputs) {
            if let Some((codes, map)) = input {
                let span = span.get_or_insert_with(Spans::default);
                if let Edges::Pairs(edges, dots) = &mut edges {
                    Cause::ScalarPairWords.mark();
                    adapt::pairs::spans_into(&codes, edges, &map, (width, height), *dots, span);
                } else {
                    fixture_stage!(FusedReduceMap, fastdraw::code_spans_into(width, height, &codes, &map, cache, span));
                }
            }
        }
        return;
    };
    let words = words.as_chunks::<N>().0;
    let offsets = lanes.channels.map(|channel| {
        channel.and_then(|c| inputs[c]).and_then(|(codes, _)| codes.corrected).map_or([0; PHASES], |cal| cal.offsets)
    });
    let correction = Correction::new(std::array::from_fn(|phase| offsets.map(|lane| lane[phase])));
    // Uncompressed Peak uses the decoder's one time step per word (see docs/architecture/frames.md).
    let pairs = matches!(edges, Edges::Pairs(..));
    match edges {
        Edges::Pairs(edges, dots) => {
            fixture_stage!(ColumnEdges, edges.update_for(width, words.len(), map, true));
            fixture_stage!(MinMax, {
                reduce_columns(words, &correction, (edges.ranges().c0, &edges.ranges().first, false), &lanes, out);
                adapt::pairs::join_codes(words, &correction, edges, (dots, map.x_per_sample), out);
            });
        }
        Edges::Record(edges) => {
            fixture_stage!(ColumnEdges, edges.update(width, words.len(), map));
            envelope(words, &correction, edges, &lanes, out);
        }
        Edges::Columns(c0, first) => {
            fixture_stage!(MinMax, reduce_columns(words, &correction, (c0, first, true), &lanes, out))
        }
    }
    write_spans(out, &inputs, (width, height), pairs, rows, spans);
}

fn write_spans<const N: usize>(
    envelopes: &Envelopes<N>,
    inputs: &[Option<(Codes<'_>, Map)>; NCH],
    (width, height): (usize, usize),
    pairs: bool,
    rows: &mut [RowCache; NCH],
    spans: &mut [Option<Spans>; NCH],
) {
    for (lane, channel) in envelopes.lanes.channels.into_iter().enumerate() {
        let Some(c) = channel else { continue };
        let Some((codes, map)) = inputs[c] else { continue };
        let span = spans[c].get_or_insert_with(Spans::default);
        fixture_stage!(CodeToRow, c, {
            spans_into(envelopes, lane, &codes, &map, height, &mut rows[c], span);
            if pairs {
                let c0 = std::mem::take(&mut span.c0);
                let len = span.rows.len();
                span.rows.resize(width, (1, 0));
                span.rows.copy_within(..len, c0);
                span.rows[..c0].fill((1, 0));
            }
        });
    }
}
