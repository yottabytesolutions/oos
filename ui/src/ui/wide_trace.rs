// SPDX-License-Identifier: GPL-2.0-or-later

use super::{rgb, Canvas, Phosphor, Rect, TraceJob, Ui, CH_COLORS, NCH};

impl TraceJob {
    #[cfg(target_os = "linux")]
    pub(crate) fn count_map<'a>(&'a self, ph: &'a Phosphor, c: usize) -> &'a [u16] {
        self.indexed.as_ref().map_or(&ph.maps()[c], |maps| &maps[c])
    }

    /// Keep counts in Picture. R16UI carries only the CPU grader's exact LUT index here.
    /// Encoding 2*i+1 preserves the distinction between an unlit pixel and a lit index zero.
    /// The existing shader's nonzero test and shift=1 recover both without a protocol change.
    pub(super) fn index_wide(&mut self, ph: &Phosphor) {
        let encode = |value: u32, shift: u32| {
            if value == 0 {
                0
            } else {
                (((value >> shift).min(1023) << 1) | 1) as u16
            }
        };
        let maps = if self.graded {
            let merged = (0..ph.w * ph.h)
                .map(|i| encode((0..NCH).filter(|&c| self.show[c]).map(|c| ph.count(c, i)).sum(), self.shift[0]))
                .collect();
            self.show = [true, false, false, false];
            let mut maps = std::array::from_fn(|_| Vec::new());
            maps[0] = merged;
            maps
        } else {
            std::array::from_fn(|c| {
                if self.show[c] {
                    (0..ph.w * ph.h).map(|i| encode(ph.count(c, i), self.shift[c])).collect()
                } else {
                    Vec::new()
                }
            })
        };
        self.indexed = Some(maps);
        self.shift = [1; NCH];
    }
}

pub(super) fn draw_traces(
    ui: &Ui,
    cv: &mut Canvas,
    area: Rect,
    ph: &Phosphor,
    vmaxs: &[u32; NCH],
    vmax_cg: u32,
    all_sources: bool,
) {
    if ui.m.color_grade {
        return draw_graded(ui, cv, area, ph, vmax_cg, all_sources);
    }
    let (w, h) = (ph.w, ph.h);
    for c in (0..NCH).rev() {
        if !ui.m.ch[c].on || (!all_sources && !ui.shows(c)) {
            continue;
        }
        let col = CH_COLORS[c];
        let (cr, cg, cb) = ((col >> 16) & 255, (col >> 8) & 255, col & 255);
        let (lut, shift, least) = ui.trace_lut(vmaxs[c]);
        let put = |p: &mut u32, v: u32| {
            if v == 0 {
                return;
            }
            let a = lut[((v as usize) >> shift).min(1023)].max(least);
            // additive with saturation, but channels on top of each other mix towards the newer one
            let d = *p;
            let r = (((d >> 16) & 255) * (255 - a) / 255 + cr * a / 255).min(255);
            let g = (((d >> 8) & 255) * (255 - a) / 255 + cg * a / 255).min(255);
            let b = ((d & 255) * (255 - a) / 255 + cb * a / 255).min(255);
            *p = rgb(r, g, b);
        };
        match Ui::stretch(area, w, h) {
            None => {
                for y in 0..h {
                    let o = ((area.y + y as i32) * cv.w + area.x) as usize;
                    for x in 0..w {
                        put(&mut cv.px[o + x], ph.count(c, y * w + x));
                    }
                }
            }
            Some((xs, ys)) => {
                for (y, &sy) in ys.iter().enumerate() {
                    let o = ((area.y + y as i32) * cv.w + area.x) as usize;
                    for (x, &sx) in xs.iter().enumerate() {
                        put(&mut cv.px[o + x], ph.count(c, sy * w + sx));
                    }
                }
            }
        }
    }
}

/// Colour-graded persistence: the hit maps of all enabled channels are added up and every pixel is coloured by its
/// density relative to the densest pixel (blue = rare .. red = frequent). Per frame only a 1024-entry LUT is
/// built; per pixel it is an add per channel and one table look-up.
pub(super) fn draw_graded(ui: &Ui, cv: &mut Canvas, area: Rect, ph: &Phosphor, vmax: u32, all_sources: bool) {
    let channels: Vec<usize> = (0..NCH).filter(|&c| ui.m.ch[c].on && (all_sources || ui.shows(c))).collect();
    if channels.is_empty() {
        return;
    }
    let (lut, shift) = ui.grade_lut(vmax);
    let w = ph.w;
    if let Some((xs, ys)) = Ui::stretch(area, w, ph.h) {
        for (y, &sy) in ys.iter().enumerate() {
            let o = ((area.y + y as i32) * cv.w + area.x) as usize;
            for (x, &sx) in xs.iter().enumerate() {
                let s: u32 = channels.iter().map(|&c| ph.count(c, sy * w + sx)).sum();
                if s != 0 {
                    cv.px[o + x] = lut[((s >> shift) as usize).min(1023)];
                }
            }
        }
        return;
    }
    for y in 0..ph.h {
        let o = ((area.y + y as i32) * cv.w + area.x) as usize;
        let row = y * w;
        for (x, d) in cv.px[o..o + w].iter_mut().enumerate() {
            let s: u32 = channels.iter().map(|&c| ph.count(c, row + x)).sum();
            if s != 0 {
                *d = lut[((s >> shift) as usize).min(1023)];
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gfx::Rect;
    use crate::picture::deposit::count::CountFormat;

    #[test]
    fn indices_preserve_zero_and_all_u32_lut_boundaries_in_the_existing_shader() {
        let values = [0, 1, 24, 65_535, 65_536, 69_216, 536_870_904];
        let mut ph = Phosphor::new(values.len(), 1);
        ph.set_format(CountFormat::Wide);
        for c in 0..NCH {
            ph.wide_map_mut(c).copy_from_slice(&values);
        }
        for graded in [false, true] {
            let shifts = [0, 3, 6, 20];
            let mut job = TraceJob {
                area: Rect::new(0, 0, values.len() as i32, 1),
                zoom: false,
                graded,
                show: [true, false, true, true],
                alpha: Vec::new(),
                colours: Vec::new(),
                shift: shifts,
                col: [0; NCH],
                indexed: None,
            };
            job.index_wide(&ph);
            let maps = job.indexed.as_ref().unwrap();
            for (i, &value) in values.iter().enumerate() {
                for c in 0..NCH {
                    if !job.show[c] {
                        assert!(maps[c].is_empty());
                        continue;
                    }
                    let original = if graded { value * 3 } else { value };
                    let shift = if graded { shifts[0] } else { shifts[c] };
                    let encoded = u32::from(maps[c][i]);
                    assert_eq!(encoded != 0, original != 0);
                    assert_eq!(encoded >> job.shift[c], (original >> shift).min(1023));
                    assert!(encoded <= 2047);
                }
            }
        }
    }
}
