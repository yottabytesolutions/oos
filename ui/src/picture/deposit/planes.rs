// SPDX-License-Identifier: GPL-2.0-or-later

use crate::model::NCH;
use std::ops::Range;

/// Column differences with a sentinel row below the picture.
pub(crate) struct DiffPlane {
    cols: usize,
    rows: usize,
    pub(crate) d: Vec<u16>,
    touched: Range<usize>,
}

/// Owned column band. No other thread can borrow its differences while it is in a pool job.
pub(crate) struct DiffBand {
    pub(crate) channel: usize,
    pub(crate) first: usize,
    pub(crate) plane: DiffPlane,
    cols: usize,
    run: Vec<u16>,
}

impl DiffPlane {
    pub(crate) fn bands((cols, rows): (usize, usize), used: [bool; NCH]) -> Vec<DiffBand> {
        let count = used.iter().filter(|&&on| on).count().max(1);
        let band_cols = cols.div_ceil(4usize.div_ceil(count)).max(1);
        used.into_iter()
            .enumerate()
            .filter(|(_, on)| *on)
            .flat_map(|(channel, _)| {
                (0..cols).step_by(band_cols).map(move |first| {
                    let width = band_cols.min(cols - first);
                    DiffBand {
                        channel,
                        first,
                        cols,
                        plane: DiffPlane { cols: width, rows, d: vec![0; width * (rows + 1)], touched: rows..0 },
                        run: vec![0; width],
                    }
                })
            })
            .collect()
    }
}

impl DiffBand {
    pub(crate) fn touch(&mut self, rows: Range<usize>) {
        self.plane.touched = self.plane.touched.start.min(rows.start)..self.plane.touched.end.max(rows.end);
    }

    pub(crate) fn clear(&mut self) {
        if self.plane.touched.start <= self.plane.touched.end {
            self.plane.d[self.plane.touched.start * self.plane.cols
                ..(self.plane.touched.end.min(self.plane.rows) + 1) * self.plane.cols]
                .fill(0);
        }
        self.plane.touched = self.plane.rows..0;
    }

    pub(crate) fn fold(&mut self, map: &mut [u16]) {
        super::kernel::fold_band(
            map,
            &mut self.plane.d,
            &mut self.run,
            (self.cols, self.plane.rows, self.first),
            self.plane.touched.clone(),
            false,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_enabled_channels_allocate_and_recycling_clears_the_sentinel() {
        for channels in [1, 2, 4] {
            let mut bands = DiffPlane::bands((97, 48), std::array::from_fn(|c| c < channels));
            assert_eq!(bands.iter().map(|b| b.plane.d.len()).sum::<usize>(), channels * 97 * 49);
            assert_eq!(bands.len(), 4);
            let mut map = vec![0; 97 * 48];
            for band in &mut bands {
                crate::fastdraw::diff_span(&mut band.plane.d, band.plane.cols, 0, 47, 47, 24);
                band.touch(47..48);
                band.fold(&mut map);
                band.clear();
                assert!(band.plane.d.iter().all(|&v| v == 0));
            }
            assert_eq!(map.iter().map(|&v| u32::from(v)).sum::<u32>(), 24 * bands.len() as u32);
        }
    }
}
