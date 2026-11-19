// SPDX-License-Identifier: GPL-2.0-or-later

use super::{trace_columns, Model, HDIV, NCH};
use std::ops::Range;

/// Hit-count maps (one per channel) of the waveform area: the digital phosphor.
pub struct Phosphor {
    pub w: usize,
    pub h: usize,
    pub map: [Vec<u16>; NCH],
    wide: Option<[Vec<u32>; NCH]>,
    occupied: Range<usize>,
    dirty: Range<usize>,
    published: Range<usize>,
}

impl Phosphor {
    pub fn new(w: usize, h: usize) -> Phosphor {
        Phosphor { w, h, map: Default::default(), wide: None, occupied: 0..0, dirty: 0..0, published: 0..0 }.init()
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
        self.set_format(crate::picture::deposit::count::CountFormat::Narrow);
        self.clear_counts();
    }

    pub(crate) fn clear_counts(&mut self) {
        self.clear_rows();
        if let Some(maps) = &mut self.wide {
            for map in maps {
                map.fill(0);
            }
        }
        for m in self.map.iter_mut() {
            m.iter_mut().for_each(|x| *x = 0);
        }
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
        for m in self.map.iter_mut() {
            for x in m.iter_mut() {
                if *x != 0 {
                    changed = true;
                    *x = ((*x as u32 * k) >> 16) as u16;
                }
            }
        }
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
        for m in self.map.iter_mut() {
            for x in m.iter_mut() {
                if *x != 0 {
                    *x = (*x >> 1).max(1);
                }
            }
        }
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
        self.touch(rows);
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
    let mut rows = ph..0;
    let put = |cx, r0: i32, r1: i32, weight| {
        let (first, last) = (r0.max(b0), r1.min(b1 - 1));
        if first > last || weight == 0 {
            return;
        }
        rows = rows.start.min(first as usize)..rows.end.max(last as usize + 1);
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
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::picture::deposit::count::CountFormat;

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
