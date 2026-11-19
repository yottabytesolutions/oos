// SPDX-License-Identifier: GPL-2.0-or-later

use super::count::{Count, CountFormat};
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
    wide: Option<(Vec<u32>, Vec<u32>)>,
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
                        wide: None,
                    }
                })
            })
            .collect()
    }

    pub(crate) fn format(&self) -> CountFormat {
        if self.wide.is_some() {
            CountFormat::Wide
        } else {
            CountFormat::Narrow
        }
    }

    pub(crate) fn set_format(&mut self, format: CountFormat) {
        if self.format() == format {
            return;
        }
        let width = self.plane.band_cols;
        match format {
            CountFormat::Narrow => {
                self.wide = None;
                self.plane.d = vec![0; width * (self.plane.rows + 1)];
                self.run = vec![0; width];
            }
            CountFormat::Wide => {
                self.wide = Some((vec![0; width * (self.plane.rows + 1)], vec![0; width]));
                self.plane.d = Vec::new();
                self.run = Vec::new();
            }
        }
        self.plane.touched = self.plane.rows..0;
    }

    #[cfg(test)]
    pub(crate) fn test_span(&mut self, row: usize, weight: u16) {
        match &mut self.wide {
            Some((diff, _)) => crate::fastdraw::diff_span(diff, self.plane.band_cols, 0, row, row, weight),
            None => crate::fastdraw::diff_span(&mut self.plane.d, self.plane.band_cols, 0, row, row, weight),
        }
        self.touch(row..row + 1);
    }

    pub(crate) fn paint(&mut self, samples: super::kernel::SamplesRef<'_>, plan: &super::standard::FastPlan) {
        let rows = match &mut self.wide {
            Some((diff, _)) => super::standard::add_sample_band(self.channel, diff, self.first, samples, plan),
            None => super::standard::add_sample_band(self.channel, &mut self.plane.d, self.first, samples, plan),
        };
        self.touch(rows);
    }

    pub(crate) fn fold_picture(&mut self, prior: [Option<&Self>; 2], ph: &mut crate::wave::Phosphor) {
        match &mut self.wide {
            Some((diff, run)) => {
                let mut touched = self.plane.touched.clone();
                let diffs = prior.map(|band| {
                    band.map(|b| {
                        touched = touched.start.min(b.plane.touched.start)..touched.end.max(b.plane.touched.end);
                        b.wide.as_ref().expect("window formats match").0.as_slice()
                    })
                });
                ph.edit_wide_channel(self.channel, |map| {
                    fold_window(diff, diffs, run, map, (self.picture_cols, self.plane.rows, self.first), touched)
                });
            }
            None if ph.is_wide() => {
                assert!(prior.iter().all(Option::is_none), "mixed window formats are evicted before folding");
                ph.edit_wide_channel(self.channel, |map| {
                    fold_window(
                        &self.plane.d,
                        [None, None],
                        &mut self.run,
                        map,
                        (self.picture_cols, self.plane.rows, self.first),
                        self.plane.touched.clone(),
                    )
                });
            }
            None if prior.iter().all(Option::is_none) => {
                ph.edit_channel(self.channel, |map| self.fold(map));
            }
            None => ph.edit_channel(self.channel, |map| self.fold_window(prior, map)),
        }
    }

    pub(crate) fn touch(&mut self, rows: Range<usize>) {
        if !rows.is_empty() {
            self.plane.touched = self.plane.touched.start.min(rows.start)..self.plane.touched.end.max(rows.end);
        }
    }

    pub(crate) fn clear(&mut self) {
        if self.plane.touched.start <= self.plane.touched.end {
            let range = self.plane.touched.start * self.plane.band_cols
                ..(self.plane.touched.end.min(self.plane.rows) + 1) * self.plane.band_cols;
            match &mut self.wide {
                Some((diff, _)) => diff[range].fill(0),
                None => self.plane.d[range].fill(0),
            }
        }
        self.plane.touched = self.plane.rows..0;
    }

    pub(crate) fn fold(&mut self, map: &mut [u16]) -> Range<usize> {
        super::kernel::fold_band(
            map,
            &mut self.plane.d,
            &mut self.run,
            (self.picture_cols, self.plane.rows, self.first),
            self.plane.touched.clone(),
            false,
        )
    }

    pub(crate) fn fold_window(&mut self, prior: [Option<&Self>; 2], map: &mut [u16]) -> Range<usize> {
        let mut touched = self.plane.touched.clone();
        let diffs = prior.map(|band| {
            band.map(|band| {
                touched = touched.start.min(band.plane.touched.start)..touched.end.max(band.plane.touched.end);
                band.plane.d.as_slice()
            })
        });
        fold_window(&self.plane.d, diffs, &mut self.run, map, (self.picture_cols, self.plane.rows, self.first), touched)
    }
}

fn fold_window<T: Count, M: Count>(
    newest: &[T],
    diffs: [Option<&[T]>; 2],
    run: &mut [T],
    map: &mut [M],
    geometry: (usize, usize, usize),
    touched: Range<usize>,
) -> Range<usize> {
    let width = run.len();
    super::kernel::fold_rows(map, run, geometry, touched.clone(), |r, run| {
        let start = r * width;
        let mut occupied = false;
        for diff in diffs.into_iter().flatten() {
            for (sum, delta) in run.iter_mut().zip(&diff[start..][..width]) {
                *sum = sum.wrapping_add(*delta);
            }
        }
        for (sum, delta) in run.iter_mut().zip(&newest[start..][..width]) {
            *sum = sum.wrapping_add(*delta);
            occupied |= *sum != T::default();
        }
        occupied
    })
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

#[cfg(test)]
mod wide_tests {
    use super::*;
    use crate::picture::deposit::{kernel, standard};

    #[test]
    fn coincident_dots_prefix_window_and_recycling_cross_u16_without_clipping() {
        let mut m = crate::model::Model { dots: crate::model::DotsMode::On, ..Default::default() };
        m.ch[0].offset = 0.0;
        let proj = standard::projection(&m, 1, 8, 1.0 / 2884.0);
        let samples = vec![0.0; 2884];
        let trace = kernel::RawTrace {
            trigger_at: 1442.0,
            outcome: kernel::TrigOutcome::Hardware,
            spans: std::array::from_fn(|_| None),
        };
        let mut sets: Vec<_> = (0..3).map(|_| DiffBand::bands((1, 8), [true, false, false, false])).collect();
        for set in &mut sets {
            let b = &mut set[0];
            b.set_format(CountFormat::Wide);
            b.touch(0..8);
            kernel::add_placed_band(
                0,
                &mut b.wide.as_mut().unwrap().0,
                0,
                &kernel::SamplesRef::Volts(&samples),
                &proj,
                &trace,
            );
            assert!(b.plane.d.is_empty() && b.run.is_empty(), "wide allocates no narrow payload");
            assert_eq!(b.wide.as_ref().unwrap().0.capacity(), 9);
            assert_eq!(b.wide.as_ref().unwrap().1.capacity(), 1);
        }
        let mut ph = crate::wave::Phosphor::new(1, 8);
        ph.set_format(CountFormat::Wide);
        let (newest, prior) = sets.split_last_mut().unwrap();
        newest[0].fold_picture([Some(&prior[0][0]), Some(&prior[1][0])], &mut ph);
        assert_eq!(ph.count(0, 4), 3 * 69_216);
        newest[0].clear();
        assert!(newest[0].wide.as_ref().unwrap().0.iter().all(|&x| x == 0));
        ph.clear_counts();
        newest[0].fold_picture([None, None], &mut ph);
        assert_eq!(ph.channel_max(0), 0);
        newest[0].set_format(CountFormat::Narrow);
        assert_eq!(newest[0].plane.d.capacity(), 9);
        assert!(newest[0].wide.is_none());
    }
}

#[cfg(test)]
mod allocation_tests {
    use super::*;
    use crate::system::test_allocator::own_high_water;

    #[test]
    fn five_wide_sets_and_picture_have_bounded_real_allocations() {
        for w in [271, 970] {
            let (peak, ()) = own_high_water(|| {
                let sets: Vec<_> = (0..5)
                    .map(|_| {
                        let mut bands = DiffBand::bands((w, 448), [true; 4]);
                        for band in &mut bands {
                            band.set_format(CountFormat::Wide);
                        }
                        bands
                    })
                    .collect();
                let payload: usize = sets
                    .iter()
                    .flatten()
                    .map(|band| {
                        assert!(band.plane.d.is_empty() && band.run.is_empty());
                        let (diff, run) = band.wide.as_ref().unwrap();
                        (diff.capacity() + run.capacity()) * size_of::<u32>()
                    })
                    .sum();
                assert_eq!(payload, 5 * 4 * w * 450 * 4);
                let mut ph = crate::wave::Phosphor::new(w, 448);
                ph.set_format(CountFormat::Wide);
                std::hint::black_box((&sets, &ph));
            });
            // Include the narrow picture temporarily alive during promotion and metadata.
            let bound = 5 * 4 * w * 450 * 4 + 4 * w * 448 * 6 + 16_384;
            println!("WIDE_HEAP width={w} peak={peak} bound={bound}");
            assert!(peak <= bound, "{peak} exceeds {bound}");
        }
    }
}
