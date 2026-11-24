// SPDX-License-Identifier: GPL-2.0-or-later

pub(crate) mod dirty;
mod rows;
pub(crate) use rows::RowBand;

use super::{trace_columns, Model, HDIV, NCH};
use std::{ops::Range, sync::OnceLock};

/// Hit-count maps (one per channel) of the waveform area: the digital phosphor.
pub struct Phosphor {
    /// Fixed at construction. Replace the phosphor to resize its maps and row revisions together.
    pub w: usize,
    /// Fixed at construction. Replace the phosphor to resize its maps and row revisions together.
    pub h: usize,
    map: [Vec<u16>; NCH],
    wide: [Vec<u32>; NCH],
    wide_active: bool,
    maxima: [OnceLock<u32>; NCH],
    dirty: dirty::Dirty,
    touched: [rows::Touched; NCH],
    published: Range<usize>,
    weight: f64,
}

impl Phosphor {
    pub fn new(w: usize, h: usize) -> Phosphor {
        Phosphor {
            w,
            h,
            map: std::array::from_fn(|_| vec![0; w * h]),
            wide: Default::default(),
            wide_active: false,
            maxima: Default::default(),
            dirty: dirty::Dirty::new(h),
            touched: std::array::from_fn(|_| rows::Touched::new(h)),
            published: 0..0,
            weight: 0.0,
        }
    }
    pub(crate) fn publish(&mut self) {
        self.published = self.dirty.publish();
    }

    /// The covering row range of count changes in the last picture publication.
    pub fn published_rows(&self) -> Range<usize> {
        self.published.clone()
    }

    pub(crate) fn is_wide(&self) -> bool {
        self.wide_active
    }

    pub(crate) fn set_format(&mut self, format: crate::picture::deposit::count::CountFormat) {
        use crate::picture::deposit::count::CountFormat;
        if self.is_wide() == (format == CountFormat::Wide) {
            return;
        }
        self.wide_active = match format {
            CountFormat::Wide => {
                for (wide, narrow) in self.wide.iter_mut().zip(&mut self.map) {
                    wide.extend(narrow.drain(..).map(u32::from));
                }
                true
            }
            CountFormat::Narrow => {
                self.clear_counts();
                for (wide, narrow) in self.wide.iter_mut().zip(&mut self.map) {
                    narrow.resize(wide.len(), 0);
                    wide.clear();
                }
                false
            }
        };
        self.dirty.invalidate();
    }

    #[cfg(test)]
    pub(crate) fn wide_map_mut(&mut self, c: usize) -> &mut [u32] {
        self.mark_rows(c, 0..self.h);
        &mut self.wide[c]
    }

    pub(crate) fn count(&self, c: usize, i: usize) -> u32 {
        match self.wide_active {
            true => self.wide[c][i],
            false => u32::from(self.map[c][i]),
        }
    }

    pub(crate) fn channel_max(&self, c: usize) -> u32 {
        *self.maxima[c].get_or_init(|| self.scan_max(c))
    }

    fn scan_max(&self, c: usize) -> u32 {
        if self.is_wide() {
            self.wide[c].iter().copied().max().unwrap_or(0)
        } else {
            u32::from(self.map[c].iter().copied().max().unwrap_or(0))
        }
    }

    pub(crate) fn near_full(&self, maximum: u32) -> bool {
        maximum > if self.is_wide() { 0x0c00_0000 } else { 0xc000 }
    }

    pub fn clear(&mut self) {
        if self.is_wide() {
            self.set_format(crate::picture::deposit::count::CountFormat::Narrow);
        } else {
            self.clear_counts();
        }
    }

    pub(crate) fn add_weight(&mut self, weight: f64) {
        self.weight += weight;
    }

    #[cfg(test)]
    pub(crate) fn weight(&self) -> f64 {
        self.weight
    }

    pub(crate) fn clear_counts(&mut self) {
        self.weight = 0.0;
        // Channels with no writes since the last clear have no counts to erase.
        for (channel, touched) in self.touched.iter_mut().enumerate() {
            let changed = match self.wide_active {
                true => touched.clear(&mut self.wide[channel], self.w),
                false => touched.clear(&mut self.map[channel], self.w),
            };
            self.dirty.mark(channel, changed);
            self.maxima[channel].take();
        }
    }

    /// Multiplies every count by f (0..1).
    pub fn decay(&mut self, f: f32) {
        let k = (f.clamp(0.0, 1.0) * 65536.0) as u32;
        if k >= 65536 {
            return;
        }
        self.weight *= f64::from(k) / 65536.0;
        self.map_rows(|value| ((u64::from(value) * u64::from(k)) >> 16) as u32);
    }

    /// Halves every count (non-zero counts stay >= 1, so rare hits survive): keeps the density ratios when the
    /// counts approach saturation under long persistence.
    pub fn halve(&mut self) {
        self.weight *= 0.5;
        self.map_rows(|value| if value == 0 { 0 } else { (value >> 1).max(1) });
    }

    /// Highest sum of the maps of the channels in `on` at one pixel (colour grade).
    pub fn merged_max(&self, on: [bool; NCH]) -> u32 {
        let mut channels = (0..NCH).filter(|&c| on[c]);
        let Some(first) = channels.next() else { return 0 };
        if channels.next().is_none() {
            return self.channel_max(first);
        }
        if self.is_wide() {
            return (0..self.w * self.h)
                .map(|i| (0..NCH).filter(|&c| on[c]).map(|c| self.count(c, i)).sum())
                .max()
                .unwrap_or(0);
        }
        let maps = std::array::from_fn::<_, NCH, _>(|c| on[c].then_some(self.map[c].as_slice()));
        (0..self.w * self.h).map(|i| maps.iter().flatten().map(|map| u32::from(map[i])).sum::<u32>()).max().unwrap_or(0)
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
        let rows = match self.wide_active {
            true => draw_trace(&mut self.wide[c], pw, ph, 0, c, m, view, rec_t0, dt, v, w, z, pairs),
            false => draw_trace(&mut self.map[c], pw, ph, 0, c, m, view, rec_t0, dt, v, w, z, pairs),
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

    pub(crate) fn edit_wide_channel(&mut self, channel: usize, apply: impl FnOnce(&mut [u32]) -> Range<usize>) {
        let rows = apply(&mut self.wide[channel]);
        self.mark_rows(channel, rows);
    }

    #[cfg(test)]
    pub(crate) fn clear_channel(&mut self, channel: usize) {
        self.map[channel].fill(0);
        self.dirty.mark(channel, 0..self.h);
        self.maxima[channel].take();
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
    fn mark_rows(&mut self, channel: usize, rows: Range<usize>) {
        self.maxima[channel].take();
        self.touched[channel].mark(rows.clone());
        self.dirty.mark(channel, rows);
    }

    pub(crate) fn upload_rows(&self, channel: usize, previous: Option<&dirty::Stamp>) -> (dirty::Stamp, Range<usize>) {
        self.dirty.since(channel, previous)
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
    fn map_rows(&mut self, mut apply: impl FnMut(u32) -> u32) {
        let width = self.w.max(1);
        for c in 0..NCH {
            let rows = match self.wide_active {
                true => map_rows(&mut self.wide[c], width, &mut apply),
                false => map_rows(&mut self.map[c], width, &mut apply),
            };
            self.dirty.mark(c, rows);
            self.maxima[c].take();
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

fn map_rows<T: crate::picture::deposit::count::Count>(
    map: &mut [T],
    width: usize,
    mut apply: impl FnMut(u32) -> u32,
) -> Range<usize> {
    let mut band = RowBand::empty();
    for (y, row) in map.chunks_mut(width).enumerate() {
        let mut changed = false;
        for value in row {
            let next = T::from_value(apply(value.value()));
            changed |= next != *value;
            *value = next;
        }
        if changed {
            band.include(y..y + 1);
        }
    }
    band.into_range()
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

#[cfg(test)]
mod tests;
