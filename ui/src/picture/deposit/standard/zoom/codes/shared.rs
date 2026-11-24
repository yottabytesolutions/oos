// SPDX-License-Identifier: GPL-2.0-or-later

use super::{Codes, EnvelopeGrid, ScreenGrid, NCH};

type Extrema = ([u16; NCH], [u16; NCH]);

/// The usual four-lane overview visits each group of sample words once per screen pair.
pub(super) fn append(
    grid: &mut EnvelopeGrid,
    key: ScreenGrid,
    samples: &[Codes<'_>; NCH],
    channels: &mut [Vec<f32>; NCH],
) -> bool {
    let Some(words) = samples[0].words else { return false };
    let (words, tail) = words.as_chunks::<NCH>();
    if !tail.is_empty()
        || key.len < 2
        || !key.n.is_multiple_of(2)
        || words.len() != key.len
        || samples.iter().enumerate().any(|(c, s)| s.stride != NCH || s.word != c || s.n != key.len)
    {
        return false;
    }
    grid.prepare(key);
    let offsets: [[i32; NCH]; 8] =
        std::array::from_fn(|phase| samples.each_ref().map(|s| s.corrected.map_or(0, |cal| cal.offsets[phase])));
    for out in &mut *channels {
        out.clear();
        out.resize(key.n, f32::NAN);
    }
    let paint = |channels: &mut [Vec<f32>; NCH], point: usize, (lo, hi): Extrema| {
        for (c, out) in channels.iter_mut().enumerate() {
            let (low, high) = samples[c].calibrated(lo[c], hi[c]);
            out[point] = low;
            out[point + 1] = high;
        }
    };
    if offsets.iter().flatten().all(|&o| o == 0) {
        crate::drawpipe::stage::fixture_stage!(Resample, walk(grid, words, channels, |_, row| *row, paint));
    } else {
        crate::drawpipe::stage::fixture_stage!(
            Resample,
            walk(grid, words, channels, |i, row| corrected(*row, offsets[i % 8]), paint)
        );
    }
    true
}

fn corrected(words: [u16; NCH], offsets: [i32; NCH]) -> [u16; NCH] {
    std::array::from_fn(|c| (i32::from(words[c]) - offsets[c]).clamp(0, 65535) as u16)
}

fn walk(
    grid: &EnvelopeGrid,
    words: &[[u16; NCH]],
    channels: &mut [Vec<f32>; NCH],
    row: impl Fn(usize, &[u16; NCH]) -> [u16; NCH],
    write: impl Fn(&mut [Vec<f32>; NCH], usize, Extrema),
) {
    for (pair, range) in grid.ranges.iter().enumerate() {
        let Some((first, rest)) = words[range.clone()].split_first() else {
            continue;
        };
        let mut lo = row(range.start, first);
        let mut hi = lo;
        for (i, group) in rest.iter().enumerate() {
            let group = row(range.start + 1 + i, group);
            lo = std::array::from_fn(|c| lo[c].min(group[c]));
            hi = std::array::from_fn(|c| hi[c].max(group[c]));
        }
        write(channels, pair * 2, (lo, hi));
    }
}
