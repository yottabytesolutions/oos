// SPDX-License-Identifier: GPL-2.0-or-later

use super::{dirty, trace_columns, Model, RowBand, HDIV, NCH};
use std::ops::Range;

/// Hit-count maps (one per channel) of the waveform area: the digital phosphor.
pub struct Phosphor {
    /// Fixed at construction. Replace the phosphor to resize its maps and row revisions together.
    pub w: usize,
    /// Fixed at construction. Replace the phosphor to resize its maps and row revisions together.
    pub h: usize,
    pub map: [Vec<u16>; NCH],
    wide: Option<[Vec<u32>; NCH]>,
    occupied: Range<usize>,
    dirty: Range<usize>,
    published: Range<usize>,
    uploads: dirty::Dirty,
}

impl Phosphor {
    pub fn new(w: usize, h: usize) -> Phosphor {
        Phosphor {
            w,
            h,
            map: Default::default(),
            wide: None,
            occupied: 0..0,
            dirty: 0..0,
            published: 0..0,
            uploads: dirty::Dirty::new(h),
        }
        .init()
    }
    fn init(mut self) -> Phosphor {
        for c in 0..NCH {
            self.map[c] = vec![0; self.w * self.h];
        }
        self
    }
    pub(crate) fn touch(&mut self, rows: Range<usize>) {
        include(&mut self.occupied, &rows);
        include(&mut self.dirty, &rows);
        for c in 0..NCH {
            self.uploads.mark(c, rows.clone());
        }
    }

    pub(crate) fn publish(&mut self) {
        self.published = std::mem::take(&mut self.dirty);
    }

    /// The covering row range of count changes in the last picture publication.
    pub fn published_rows(&self) -> Range<usize> {
        self.published.clone()
    }

    fn clear_rows(&mut self) {
        include(&mut self.dirty, &std::mem::take(&mut self.occupied));
    }
    pub(crate) fn is_wide(&self) -> bool {
        self.wide.is_some()
    }

    pub(crate) fn set_format(&mut self, format: crate::picture::deposit::count::CountFormat) {
        use crate::picture::deposit::count::CountFormat;
        if self.is_wide() == (format == CountFormat::Wide) {
            return;
        }
        match format {
            CountFormat::Wide => {
                self.wide = Some(std::array::from_fn(|c| self.map[c].drain(..).map(u32::from).collect()));
                self.map = Default::default();
            }
            CountFormat::Narrow => {
                self.clear_rows();
                self.wide = None;
                self.map = std::array::from_fn(|_| vec![0; self.w * self.h]);
            }
        }
    }

    pub(crate) fn wide_map_mut(&mut self, c: usize) -> &mut [u32] {
        &mut self.wide.as_mut().expect("wide deposit selects wide picture storage")[c]
    }

    pub(crate) fn count(&self, c: usize, i: usize) -> u32 {
        match &self.wide {
            Some(maps) => maps[c][i],
            None => u32::from(self.map[c][i]),
        }
    }

    pub(crate) fn channel_max(&self, c: usize) -> u32 {
        match &self.wide {
            Some(maps) => maps[c].iter().copied().max().unwrap_or(0),
            None => self.map[c].iter().copied().max().unwrap_or(0) as u32,
        }
    }

    pub(crate) fn near_full(&self, maximum: u32) -> bool {
        maximum > if self.is_wide() { 0x0c00_0000 } else { 0xc000 }
    }

    pub fn clear(&mut self) {
        if self.is_wide() {
            self.set_format(crate::picture::deposit::count::CountFormat::Narrow);
            for c in 0..NCH {
                self.uploads.mark(c, 0..self.h);
            }
        }
        self.clear_counts();
    }

    pub(crate) fn clear_counts(&mut self) {
        if let Some(maps) = &mut self.wide {
            for map in maps {
                map.fill(0);
            }
        }
        let width = self.w.max(1);
        for c in 0..NCH {
            self.edit_channel(c, |map| {
                let rows = map.iter().position(|&value| value != 0).map_or(0..0, |first| {
                    let last = map.iter().rposition(|&value| value != 0).unwrap_or(first);
                    first / width..last / width + 1
                });
                map.fill(0);
                rows
            });
        }
        self.clear_rows();
    }

    /// Multiplies every count by f (0..1).
    pub fn decay(&mut self, f: f32) {
        let k = (f.clamp(0.0, 1.0) * 65536.0) as u32;
        if k >= 65536 {
            return;
        }
        let mut changed = false;
        if let Some(maps) = &mut self.wide {
            for x in maps.iter_mut().flatten() {
                changed |= *x != 0;
                *x = ((u64::from(*x) * u64::from(k)) >> 16) as u32;
            }
        }
        self.map_rows(|row| {
            let changed = row.iter().any(|&value| value != 0);
            for value in row {
                *value = ((*value as u32 * k) >> 16) as u16;
            }
            changed
        });
        if changed {
            include(&mut self.dirty, &self.occupied);
        }
    }

    /// Halves every count (non-zero counts stay >= 1, so rare hits survive): keeps the density ratios when the
    /// counts approach saturation under long persistence.
    pub fn halve(&mut self) {
        include(&mut self.dirty, &self.occupied);
        if let Some(maps) = &mut self.wide {
            for x in maps.iter_mut().flatten().filter(|x| **x != 0) {
                *x = (*x >> 1).max(1);
            }
        }
        self.map_rows(|row| {
            let changed = row.iter().any(|&value| value > 1);
            for value in row {
                *value = if *value == 0 { 0 } else { (*value >> 1).max(1) };
            }
            changed
        });
    }

    /// Highest sum of the maps of the channels in `on` at one pixel (colour grade).
    pub fn merged_max(&self, on: [bool; NCH]) -> u32 {
        if self.is_wide() {
            return (0..self.w * self.h)
                .map(|i| (0..NCH).filter(|&c| on[c]).map(|c| self.count(c, i)).sum())
                .max()
                .unwrap_or(0);
        }
        let maps: Vec<&[u16]> = (0..NCH).filter(|&c| on[c]).map(|c| self.map[c].as_slice()).collect();
        match maps.len() {
            0 => 0,
            1 => maps[0].iter().copied().max().unwrap_or(0) as u32,
            _ => (0..self.w * self.h).map(|i| maps.iter().map(|m| m[i] as u32).sum::<u32>()).max().unwrap_or(0),
        }
    }

    /// Draws a record channel into the map: every segment between two samples adds `w` to the pixels it covers,
    /// so the brightness follows the dwell time of the trace like on an analog/digital-phosphor screen. Display
    /// options: dots (`Model::dots_at`, one pixel per sample) and per-sample brightness weights 0..1 (Z input).
    /// `pairs` (`Record::pairs`): with dots, each min/max pair is one vertical span, since it stands for every sample
    /// between its extremes; two dots would drop the edges of a slow time base.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_ex(
        &mut self,
        c: usize,
        m: &Model,
        view: (f32, f32),
        rec_t0: f64,
        dt: f64,
        v: &[f32],
        w: u16,
        z: Option<&[f32]>,
        pairs: bool,
    ) {
        let (pw, ph) = (self.w, self.h);
        let rows = match &mut self.wide {
            Some(maps) => draw_trace(&mut maps[c], pw, ph, 0, c, m, view, rec_t0, dt, v, w, z, pairs),
            None => draw_trace(&mut self.map[c], pw, ph, 0, c, m, view, rec_t0, dt, v, w, z, pairs),
        };
        self.mark_rows(c, rows);
    }

    pub fn maps(&self) -> &[Vec<u16>; NCH] {
        &self.map
    }

    #[cfg(test)]
    pub(crate) fn channel_mut(&mut self, channel: usize) -> &mut [u16] {
        self.mark_rows(channel, 0..self.h);
        &mut self.map[channel]
    }

    /// Edits one channel; the closure returns the range of rows it changed.
    pub(crate) fn edit_channel(&mut self, channel: usize, apply: impl FnOnce(&mut [u16]) -> Range<usize>) {
        let rows = apply(&mut self.map[channel]);
        self.mark_rows(channel, rows);
    }

    #[cfg(test)]
    pub(crate) fn clear_channel(&mut self, channel: usize) {
        self.map[channel].fill(0);
        self.mark_rows(channel, 0..self.h);
    }

    #[cfg(test)]
    pub(crate) fn add_maps(&mut self, src: &Self, used: [bool; NCH]) {
        for (c, used) in used.into_iter().enumerate() {
            if used {
                let rows = src.map[c].chunks(src.w.max(1)).map(|row| row.iter().copied());
                self.transform_channel(c, rows, u16::saturating_add);
            }
        }
    }

    #[inline]
    pub(crate) fn mark_rows(&mut self, channel: usize, rows: Range<usize>) {
        include(&mut self.occupied, &rows);
        include(&mut self.dirty, &rows);
        self.uploads.mark(channel, rows);
    }

    pub(crate) fn upload_rows(&self, channel: usize, previous: Option<&dirty::Stamp>) -> (dirty::Stamp, Range<usize>) {
        self.uploads.since(channel, previous)
    }

    pub(crate) fn fold_rows(
        &mut self,
        accumulators: &mut [Vec<u16>; NCH],
        channels: &[usize],
        band_cols: usize,
        rows: [(usize, usize); NCH],
    ) {
        for &c in channels {
            self.mark_rows(c, rows[c].0..rows[c].1);
        }
        let (w, h) = (self.w, self.h);
        let pairs: Vec<_> =
            self.map.iter_mut().zip(accumulators).enumerate().filter(|(c, _)| channels.contains(c)).collect();
        crate::fastdraw::par_each(pairs, |(c, (map, acc))| {
            crate::fastdraw::fold_t(map, acc, w, h, band_cols, rows[c].0..rows[c].1)
        });
    }

    /// Track changed rows while applying an in-place unary operation.
    fn map_rows(&mut self, mut apply: impl FnMut(&mut [u16]) -> bool) {
        let width = self.w.max(1);
        for c in 0..NCH {
            self.edit_channel(c, |map| {
                let mut band = RowBand::empty();
                for (y, row) in map.chunks_mut(width).enumerate() {
                    if apply(row) {
                        band.include(y..y + 1);
                    }
                }
                band.into_range()
            });
        }
    }

    #[cfg(test)]
    fn transform_channel<I: IntoIterator<Item = u16>>(
        &mut self,
        channel: usize,
        rows: impl IntoIterator<Item = I>,
        mut apply: impl FnMut(u16, u16) -> u16,
    ) {
        let width = self.w.max(1);
        self.edit_channel(channel, |map| {
            let mut band = RowBand::empty();
            for (y, (row, source)) in map.chunks_mut(width).zip(rows).enumerate() {
                let mut changed = false;
                for (value, input) in row.iter_mut().zip(source) {
                    let next = apply(*value, input);
                    changed |= next != *value;
                    *value = next;
                }
                if changed {
                    band.include(y..y + 1);
                }
            }
            band.into_range()
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::picture::deposit::count::CountFormat;

    #[test]
    fn publication_keeps_consumer_history_and_channel_edits_independent() {
        let mut ph = Phosphor::new(4, 16);
        let (first, _) = ph.upload_rows(0, None);
        let (other, _) = ph.upload_rows(1, None);
        ph.edit_channel(0, |map| {
            map[8] = 24;
            2..3
        });
        ph.publish();
        assert_eq!(ph.published_rows(), 2..3);
        ph.publish();
        assert!(ph.published_rows().is_empty());
        assert_eq!(ph.upload_rows(0, Some(&first)).1, 2..3);
        assert_eq!(ph.upload_rows(1, Some(&other)).1, 0..0);
        ph.clear_counts();
        ph.publish();
        assert_eq!(ph.published_rows(), 2..3);
        ph.clear_counts();
        ph.publish();
        assert!(ph.published_rows().is_empty());
    }

    #[test]
    fn folded_picture_rows_survive_publication_until_each_consumer_uploads() {
        let mut ph = Phosphor::new(4, 16);
        let (first, _) = ph.upload_rows(0, None);
        ph.map[0][8] = 24;
        ph.touch(2..3);
        ph.publish();
        assert_eq!(ph.published_rows(), 2..3);
        assert_eq!(ph.upload_rows(0, Some(&first)).1, 2..3);
        let (panel, _) = ph.upload_rows(0, Some(&first));
        ph.map[0][48] = 24;
        ph.touch(12..13);
        ph.publish();
        assert_eq!(ph.published_rows(), 12..13);
        assert_eq!(ph.upload_rows(0, Some(&panel)).1, 12..13);
        assert_eq!(ph.upload_rows(0, Some(&first)).1, 2..13);
    }

    #[test]
    fn clearing_wide_storage_invalidates_previous_narrow_upload_rows() {
        let mut ph = Phosphor::new(4, 16);
        ph.edit_channel(0, |map| {
            map[8] = 24;
            2..3
        });
        ph.publish();
        let (narrow, _) = ph.upload_rows(0, None);
        ph.set_format(CountFormat::Wide);
        ph.wide_map_mut(0)[48] = 70_000;
        ph.touch(12..13);
        ph.clear();
        ph.publish();
        assert_eq!(ph.published_rows(), 2..13);
        assert!(!ph.is_wide());
        assert!(ph.maps().iter().flatten().all(|&value| value == 0));
        assert_eq!(ph.upload_rows(0, Some(&narrow)).1, 0..16);
    }

    #[test]
    fn fully_decayed_traces_stop_publishing_rows() {
        for format in [CountFormat::Narrow, CountFormat::Wide] {
            let mut ph = Phosphor::new(2, 4);
            ph.map[NCH - 1][4] = 2;
            ph.touch(2..3);
            ph.set_format(format);
            ph.publish();
            assert_eq!(ph.published_rows(), 2..3);
            ph.decay(1.0);
            ph.publish();
            assert!(ph.published_rows().is_empty());
            for expected in [1, 0] {
                ph.decay(0.5);
                ph.publish();
                assert_eq!(ph.count(NCH - 1, 4), expected);
                assert_eq!(ph.published_rows(), 2..3);
            }
            for factor in [0.5, 0.0, 0.5] {
                ph.decay(factor);
                ph.publish();
                assert!(ph.published_rows().is_empty());
            }
        }
    }
}

fn include(hull: &mut Range<usize>, rows: &Range<usize>) {
    if !rows.is_empty() {
        *hull = if hull.start == hull.end { rows.clone() } else { hull.start.min(rows.start)..hull.end.max(rows.end) };
    }
}

/// `Phosphor::draw_ex` on one band of one channel's map (`pw` x `ph`, row-major): `band` holds the rows `r_lo` ..
/// `r_lo + band.len() / pw` (the whole map: `r_lo` 0).
#[allow(clippy::too_many_arguments)]
fn draw_trace<T: crate::picture::deposit::count::Count>(
    band: &mut [T],
    pw: usize,
    ph: usize,
    r_lo: usize,
    c: usize,
    m: &Model,
    view: (f32, f32),
    rec_t0: f64,
    dt: f64,
    v: &[f32],
    w: u16,
    z: Option<&[f32]>,
    pairs: bool,
) -> std::ops::Range<usize> {
    let (b0, b1) = (r_lo as i32, (r_lo + band.len() / pw.max(1)).min(ph) as i32);
    let wi = pw as i32;
    let mut rows = RowBand::empty();
    let put = |cx, r0: i32, r1: i32, weight| {
        let (first, last) = (r0.max(b0), r1.min(b1 - 1));
        if first > last || weight == 0 {
            return;
        }
        rows.include(first as usize..last as usize + 1);
        for r in first..=last {
            let idx = ((r - b0) * wi + cx) as usize;
            band[idx] = band[idx].saturating_add(T::weight(weight));
        }
    };
    let ch = &m.ch[c];
    let vertical = (ch.volts_per_div(), ch.offset);
    let view = (view.0 as f64, view.1 as f64);
    let dots = m.dots_at(dt / view.0 * (pw as f32 as f64 / HDIV as f64));
    trace_columns(pw, ph, vertical, dots, view, rec_t0, dt, v, w, z, pairs, (0, wi), put);
    rows.into_range()
}
