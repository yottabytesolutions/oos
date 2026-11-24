// SPDX-License-Identifier: GPL-2.0-or-later
//! Column extrema on interleaved codes, before calibration and placement.

use crate::fastdraw::code_rows::RowCache;
use crate::fastdraw::{self, Codes, Map, Spans};
use oos_core::model::NCH;
#[cfg(any(test, not(target_arch = "aarch64")))]
use std::ops::Range;

pub(crate) mod adapt;
mod edges;
pub(crate) use edges::ColumnEdges;
#[cfg(target_arch = "aarch64")]
mod neon;
mod placement;
#[cfg(any(test, not(target_arch = "aarch64")))]
mod scalar;
pub(crate) use placement::{CodePlacement, Layout, Scratch};
#[cfg(test)]
mod tests;

const PHASES: usize = 8;

/// Physical word index to zero-based channel number.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LaneMap<const N: usize> {
    channels: [Option<usize>; N],
}

impl<const N: usize> LaneMap<N> {
    /// Invert the channel-to-word table supplied by the board.
    pub(crate) fn new(words: [Option<usize>; NCH], mask: u32) -> Option<Self> {
        assert!(matches!(N, 1 | 2 | 4));
        let mut channels = [None; N];
        for (channel, word) in words.into_iter().enumerate() {
            if let Some(lane) = word.filter(|_| mask & (1 << channel) != 0) {
                let mapped = channels.get_mut(lane)?;
                if mapped.replace(channel).is_some() {
                    return None;
                }
            }
        }
        Some(Self { channels })
    }

    pub(crate) fn channel(&self, lane: usize) -> Option<usize> {
        self.channels[lane]
    }
}

/// Record-relative offsets in phase and physical lane order.
#[derive(Debug)]
pub(crate) struct Correction<const N: usize> {
    offsets: [[i32; N]; PHASES],
    active: bool,
}

impl<const N: usize> Correction<N> {
    pub(crate) fn new(offsets: [[i32; N]; PHASES]) -> Self {
        let active = offsets.iter().flatten().any(|&offset| offset != 0);
        Self { offsets, active }
    }

    #[inline]
    fn row(&self, sample: [u16; N], index: usize) -> [u16; N] {
        let phase = self.offsets[index % PHASES];
        std::array::from_fn(|lane| (i32::from(sample[lane]).saturating_sub(phase[lane])).clamp(0, 65535) as u16)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Extrema<const N: usize> {
    low: [u16; N],
    high: [u16; N],
}

impl<const N: usize> Extrema<N> {
    const EMPTY: Self = Self { low: [u16::MAX; N], high: [u16::MIN; N] };
}

/// Reusable column storage. Only lanes present in the lane map represent channels.
#[derive(Debug)]
pub(crate) struct Envelopes<const N: usize> {
    lanes: LaneMap<N>,
    c0: usize,
    columns: Vec<Extrema<N>>,
}

impl<const N: usize> Envelopes<N> {
    pub(crate) fn with_capacity(width: usize) -> Self {
        Self { lanes: LaneMap { channels: [None; N] }, c0: 0, columns: Vec::with_capacity(width) }
    }
}

/// Reduce a record with one column walk for all enabled lanes. Preallocate both buffers to the screen width
/// to keep their capacity across changes of record length and screen position.
pub(crate) fn envelope<const N: usize>(
    words: &[[u16; N]],
    correction: &Correction<N>,
    edges: &ColumnEdges,
    lanes: &LaneMap<N>,
    out: &mut Envelopes<N>,
) {
    crate::drawpipe::stage::fixture_stage!(
        MinMax,
        reduce_columns(words, correction, (edges.ranges().c0, &edges.ranges().first, true), lanes, out)
    );
}

fn reduce_columns<const N: usize>(
    words: &[[u16; N]],
    correction: &Correction<N>,
    (c0, edges, join): (usize, &[usize], bool),
    lanes: &LaneMap<N>,
    out: &mut Envelopes<N>,
) {
    out.lanes = *lanes;
    out.c0 = c0;
    out.columns.clear();
    #[cfg(target_arch = "aarch64")]
    neon::reduce(words, correction, edges, join, &mut out.columns);
    #[cfg(not(target_arch = "aarch64"))]
    out.columns.extend(
        edges.array_windows().map(|&[start, end]| scalar::reduce(words, correction, joined(start, end, join)..end)),
    );
}

#[inline]
fn joined(start: usize, end: usize, join: bool) -> usize {
    start.saturating_sub(usize::from(join && start < end))
}

/// Convert one enabled lane to the same rows as the existing code sampler.
pub(crate) fn spans_into<const N: usize>(
    envelopes: &Envelopes<N>,
    lane: usize,
    codes: &Codes<'_>,
    map: &Map,
    height: usize,
    cache: &mut RowCache,
    out: &mut Spans,
) {
    out.c0 = 0;
    out.rows.clear();
    if height == 0 || envelopes.lanes.channel(lane).is_none() {
        return;
    }
    out.c0 = envelopes.c0;
    if let Some(rows) = cache.get(codes, map, height) {
        rows.write_spans(envelopes.columns.iter().map(|column| (column.low[lane], column.high[lane])), &mut out.rows);
    } else {
        out.rows.extend(envelopes.columns.iter().map(|column| {
            let (low, high) = (column.low[lane], column.high[lane]);
            if low <= high {
                fastdraw::mapped_rows(codes.calibrated(low, high), None, map, height)
            } else {
                (1, 0)
            }
        }));
    }
}
