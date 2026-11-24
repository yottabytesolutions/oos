// SPDX-License-Identifier: GPL-2.0-or-later
//! Exact code-to-row lookup for a fixed calibration and vertical projection.

use super::{mapping, Codes, Map};

const SHIFT: u32 = 6;
const BLOCK: u16 = 1 << SHIFT;
const BUCKETS: usize = 1 << (u16::BITS - SHIFT);
const ENTRIES: usize = 2 * BUCKETS;
const SPILL: u32 = 1 << 31;

#[derive(Default)]
pub(crate) struct RowCache {
    // Envelope scratch has one cache per channel. Keep two projections/calibrations warm per channel.
    entries: [Entry; 2],
    #[cfg(test)]
    rebuilds: usize,
}

#[derive(Default)]
struct Entry {
    key: Option<Key>,
    table: Option<RowTable>,
    valid: bool,
}

#[derive(PartialEq, Eq)]
struct Key {
    calibration: [u64; 2],
    decoder: usize,
    vertical: [u32; 2],
    height: usize,
}

impl RowCache {
    pub(crate) fn get(&mut self, codes: &Codes<'_>, map: &Map, height: usize) -> Option<&RowTable> {
        // The phase offsets are applied before lookup. Only the conversion of the resulting code is cached.
        let (calibration, decoder) = codes.corrected.map_or_else(
            || ([u64::from(codes.affine.0.to_bits()), u64::from(codes.affine.1.to_bits())], codes.volt as usize),
            |cal| ([cal.scale.to_bits(), cal.offset.to_bits()], 0),
        );
        let key = Key { calibration, decoder, vertical: [map.y_per_volt.to_bits(), map.y0.to_bits()], height };
        if let Some(hit) = self.entries.iter().position(|entry| entry.key.as_ref() == Some(&key)) {
            self.entries[..=hit].rotate_right(1);
        } else {
            #[cfg(test)]
            {
                self.rebuilds += 1;
            }
            self.entries.rotate_right(1);
            let entry = &mut self.entries[0];
            entry.key = Some(key);
            entry.valid = RowTable::build(codes, map, height, &mut entry.table);
        }
        let entry = &self.entries[0];
        entry.table.as_ref().filter(|_| entry.valid)
    }

    #[cfg(test)]
    pub(crate) fn rebuilds(&self) -> usize {
        self.rebuilds
    }
}

pub(crate) struct RowTable {
    entries: Box<[u32; ENTRIES]>,
    height: i32,
    reverse: u16,
    index: Index,
}

enum Index {
    Inline,
    Mixed,
    Runs(usize),
}

impl RowTable {
    fn build(codes: &Codes<'_>, map: &Map, height: usize, table: &mut Option<Self>) -> bool {
        if !(1..=32768).contains(&height) {
            return false;
        }
        let height = height as i32;
        let (first, last) = (codes.volts(0), codes.volts(u16::MAX));
        if !first.is_finite() || !last.is_finite() || !map.y_per_volt.is_finite() || !map.y0.is_finite() {
            return false;
        }
        let row = |code| (project(codes.volts(code), map).clamp(-1, height) + 1) as u16;
        let reverse = if row(0) > row(u16::MAX) { u16::MAX } else { 0 };
        let row = |code| row(code ^ reverse);
        let table = table.get_or_insert_with(|| Self {
            entries: vec![0; ENTRIES].into_boxed_slice().try_into().expect("allocated exactly ENTRIES entries"),
            height,
            reverse,
            index: Index::Inline,
        });
        table.height = height;
        table.reverse = reverse;
        let Some(count) = build_runs(&mut table.entries[..], row) else { return false };
        table.index =
            if count <= BUCKETS && height < i16::MAX as i32 { table.index_runs(count) } else { Index::Runs(count) };
        true
    }

    fn index_runs(&mut self, count: usize) -> Index {
        self.entries.copy_within(..count, BUCKETS);
        let (buckets, runs) = self.entries.split_at_mut(BUCKETS);
        let (mut last, mut mode) = (0, Index::Inline);
        for (index, entry) in buckets.iter_mut().enumerate() {
            let start = (index as u32) << SHIFT;
            last += usize::from(last + 1 < count && code(runs[last + 1]) == start);
            let first = last;
            while last + 1 < count && code(runs[last + 1]) < start + u32::from(BLOCK) {
                last += 1;
            }
            let row = u32::from(runs[first] as u16);
            if first == last || runs[last] as u16 == row as u16 + 1 {
                let cut = if first == last { start + u32::from(BLOCK) } else { code(runs[last]) };
                // Adding the code produces a carry precisely at the transition.
                *entry = (row << 16) | (65536 - cut);
            } else {
                *entry = SPILL | (first as u32) << 16 | (last - first + 1) as u32;
                mode = Index::Mixed;
            }
        }
        mode
    }

    #[inline]
    pub(crate) fn row(&self, code: u16) -> i32 {
        let code = code ^ self.reverse;
        match self.index {
            Index::Inline => self.bucket_row::<false>(code),
            Index::Mixed => self.bucket_row::<true>(code),
            Index::Runs(count) => search(code, &self.entries[..count]),
        }
    }

    #[inline]
    fn bucket_row<const MIXED: bool>(&self, code: u16) -> i32 {
        let entry = self.entries[usize::from(code >> SHIFT)];
        if MIXED && entry & SPILL != 0 {
            let first = BUCKETS + ((entry ^ SPILL) >> 16) as usize;
            search(code, &self.entries[first..first + (entry & 0xffff) as usize])
        } else {
            ((entry + u32::from(code)) >> 16) as i32 - 1
        }
    }

    #[inline]
    pub(crate) fn write_spans(&self, pairs: impl Iterator<Item = (u16, u16)>, out: &mut Vec<(i16, i16)>) {
        match self.index {
            Index::Inline => self.write_buckets::<false>(pairs, out),
            Index::Mixed => self.write_buckets::<true>(pairs, out),
            Index::Runs(_) => out.extend(pairs.map(|(a, b)| if a <= b { self.span(a, b) } else { (1, 0) })),
        }
    }

    #[inline]
    fn write_buckets<const MIXED: bool>(&self, pairs: impl Iterator<Item = (u16, u16)>, out: &mut Vec<(i16, i16)>) {
        let span = |a, b| {
            if a <= b {
                mapping::clip_rows(self.bucket_row::<MIXED>(a), self.bucket_row::<MIXED>(b), self.height)
            } else {
                (1, 0)
            }
        };
        if self.reverse == 0 {
            out.extend(pairs.map(|(a, b)| span(a, b)));
        } else {
            out.extend(pairs.map(|(a, b)| span(!b, !a)));
        }
    }

    #[inline]
    pub(crate) fn span(&self, low: u16, high: u16) -> (i16, i16) {
        let (a, b) = (self.row(low), self.row(high));
        mapping::clip_rows(a.min(b), a.max(b), self.height)
    }
}

#[cold]
fn search(input: u16, runs: &[u32]) -> i32 {
    let index = runs.partition_point(|&run| code(run) <= u32::from(input));
    i32::from(runs[index - 1] as u16) - 1
}

fn code(run: u32) -> u32 {
    run >> 16
}

fn build_runs(entries: &mut [u32], row: impl Fn(u16) -> u16) -> Option<usize> {
    let mut start = 0;
    let mut value = row(0);
    let last = row(u16::MAX);
    for (index, entry) in entries.iter_mut().enumerate() {
        *entry = u32::from(start) << 16 | u32::from(value);
        if value == last {
            return Some(index + 1);
        }
        let (mut lo, mut hi) = (start + 1, u16::MAX);
        while lo < hi {
            let mid = lo.midpoint(hi);
            if row(mid) == value {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        start = lo;
        value = row(start);
    }
    None
}

#[inline]
pub(crate) fn project(volts: f32, map: &Map) -> i32 {
    mapping::project(volts, map).round() as i32
}

#[cfg(test)]
mod tests;
