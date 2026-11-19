// SPDX-License-Identifier: GPL-2.0-or-later

use super::{draw_trace, Model, NCH};

/// Hit-count maps (one per channel) of the waveform area: the digital phosphor.
pub struct Phosphor {
    pub w: usize,
    pub h: usize,
    pub map: [Vec<u16>; NCH],
    wide: Option<[Vec<u32>; NCH]>,
}

impl Phosphor {
    pub fn new(w: usize, h: usize) -> Phosphor {
        Phosphor { w, h, map: Default::default(), wide: None }.init()
    }
    fn init(mut self) -> Phosphor {
        for c in 0..NCH {
            self.map[c] = vec![0; self.w * self.h];
        }
        self
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
        if let Some(maps) = &mut self.wide {
            for x in maps.iter_mut().flatten() {
                *x = ((u64::from(*x) * u64::from(k)) >> 16) as u32;
            }
        }
        for m in self.map.iter_mut() {
            for x in m.iter_mut() {
                if *x != 0 {
                    *x = ((*x as u32 * k) >> 16) as u16;
                }
            }
        }
    }

    /// Halves every count (non-zero counts stay >= 1, so rare hits survive): keeps the density ratios when the
    /// counts approach saturation under long persistence.
    pub fn halve(&mut self) {
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
        match &mut self.wide {
            Some(maps) => draw_trace(&mut maps[c], pw, ph, 0, c, m, view, rec_t0, dt, v, w, z, pairs),
            None => draw_trace(&mut self.map[c], pw, ph, 0, c, m, view, rec_t0, dt, v, w, z, pairs),
        }
    }
}
