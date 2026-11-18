// SPDX-License-Identifier: GPL-2.0-or-later

use crate::model::NCH;
use std::ops::Range;

/// Column differences with a sentinel row below the picture.
pub(crate) struct DiffPlane {
    band_cols: usize,
    rows: usize,
    pub(crate) d: Vec<u16>,
    touched: Range<usize>,
}

/// Owned column band. No other thread can borrow its differences while it is in a pool job.
pub(crate) struct DiffBand {
    pub(crate) channel: usize,
    pub(crate) first: usize,
    pub(crate) plane: DiffPlane,
    picture_cols: usize,
    run: Vec<u16>,
}

impl DiffBand {
    pub(crate) fn bands((cols, rows): (usize, usize), used: [bool; NCH]) -> Vec<Self> {
        let count = used.iter().filter(|&&on| on).count().max(1);
        let band_cols = cols.div_ceil(4usize.div_ceil(count)).max(1);
        used.into_iter()
            .enumerate()
            .filter(|(_, on)| *on)
            .flat_map(|(channel, _)| {
                (0..cols).step_by(band_cols).map(move |first| {
                    let width = band_cols.min(cols - first);
                    Self {
                        channel,
                        first,
                        picture_cols: cols,
                        plane: DiffPlane { band_cols: width, rows, d: vec![0; width * (rows + 1)], touched: rows..0 },
                        run: vec![0; width],
                    }
                })
            })
            .collect()
    }

    pub(crate) fn touch(&mut self, rows: Range<usize>) {
        self.plane.touched = self.plane.touched.start.min(rows.start)..self.plane.touched.end.max(rows.end);
    }

    pub(crate) fn clear(&mut self) {
        if self.plane.touched.start <= self.plane.touched.end {
            self.plane.d[self.plane.touched.start * self.plane.band_cols
                ..(self.plane.touched.end.min(self.plane.rows) + 1) * self.plane.band_cols]
                .fill(0);
        }
        self.plane.touched = self.plane.rows..0;
    }

    pub(crate) fn fold(&mut self, map: &mut [u16]) {
        super::kernel::fold_band(
            map,
            &mut self.plane.d,
            &mut self.run,
            (self.picture_cols, self.plane.rows, self.first),
            self.plane.touched.clone(),
            false,
        );
    }

    pub(crate) fn fold_window(&mut self, prior: [Option<&Self>; 2], map: &mut [u16]) {
        let mut touched = self.plane.touched.clone();
        let diffs = prior.map(|band| {
            band.map(|band| {
                touched = touched.start.min(band.plane.touched.start)..touched.end.max(band.plane.touched.end);
                band.plane.d.as_slice()
            })
        });
        let width = self.plane.band_cols;
        let run = &mut self.run;
        let newest = &self.plane.d;
        let geometry = (self.picture_cols, self.plane.rows, self.first);
        super::kernel::fold_rows(map, run, geometry, touched, |r, run| {
            let start = r * width;
            let mut occupied = 0;
            for diff in diffs.into_iter().flatten() {
                for (sum, delta) in run.iter_mut().zip(&diff[start..][..width]) {
                    *sum = sum.wrapping_add(*delta);
                }
            }
            for (sum, delta) in run.iter_mut().zip(&newest[start..][..width]) {
                *sum = sum.wrapping_add(*delta);
                occupied |= *sum;
            }
            occupied
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_enabled_channels_allocate_and_recycling_clears_the_sentinel() {
        for channels in [1, 2, 4] {
            let mut bands = DiffBand::bands((97, 48), std::array::from_fn(|c| c < channels));
            assert_eq!(bands.iter().map(|b| b.plane.d.len()).sum::<usize>(), channels * 97 * 49);
            assert_eq!(bands.len(), 4);
            let mut map = vec![0; 97 * 48];
            for band in &mut bands {
                crate::fastdraw::diff_span(&mut band.plane.d, band.plane.band_cols, 0, 47, 47, 24);
                band.touch(47..48);
                band.fold(&mut map);
                band.clear();
                assert!(band.plane.d.iter().all(|&v| v == 0));
            }
            assert_eq!(map.iter().map(|&v| u32::from(v)).sum::<u32>(), 24 * bands.len() as u32);
        }
    }
}
