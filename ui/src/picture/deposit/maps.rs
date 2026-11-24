// SPDX-License-Identifier: GPL-2.0-or-later

use super::count::CountFormat;
use super::{Deposit, DiffBand, Maps};
use crate::model::NCH;
use crate::picture::PerMap;
use crate::wave::Phosphor;

impl Maps {
    pub(super) fn new(gen: u64, sizes: PerMap<(usize, usize)>, used: [bool; NCH]) -> Self {
        Self { gen, sizes, zoom: sizes.zoom().map(|size| DiffBand::bands(size, used)), weight: Self::zero(sizes) }
    }

    fn zero(sizes: PerMap<(usize, usize)>) -> PerMap<f64> {
        PerMap::new(0.0, sizes.zoom().map(|_| 0.0))
    }

    pub(super) fn reset(&mut self) {
        self.weight = Self::zero(self.sizes);
    }
}

impl Deposit {
    pub(crate) fn sizes(&self) -> PerMap<(usize, usize)> {
        self.maps.sizes
    }

    pub(crate) fn zoom_planes(&self) -> Option<&[DiffBand]> {
        self.maps.zoom.as_deref()
    }

    pub(crate) fn zoom_planes_mut(&mut self) -> Option<&mut Vec<DiffBand>> {
        self.maps.zoom.as_mut()
    }

    pub(super) fn all_bands_mut(&mut self) -> impl Iterator<Item = &mut DiffBand> {
        self.planes.iter_mut().chain(self.maps.zoom.iter_mut().flatten())
    }

    pub(crate) fn sort_bands(&mut self) {
        self.planes.sort_unstable_by_key(|band| (band.channel, band.first));
        if let Some(zoom) = self.zoom_planes_mut() {
            zoom.sort_unstable_by_key(|band| (band.channel, band.first));
        }
    }

    pub(crate) fn weights(&self) -> PerMap<f64> {
        self.maps.weight
    }

    pub(crate) fn add_weights(&mut self, weight: PerMap<f64>) {
        let main = self.maps.weight.main() + weight.main();
        let zoom = self.maps.weight.zoom().zip(weight.zoom()).map(|(a, b)| a + b);
        self.maps.weight = PerMap::new(main, zoom);
    }

    pub(crate) fn blend_picture(&mut self, maps: &mut PerMap<&mut Phosphor>, clear: bool) {
        let format = self.format();
        let weight = self.weights();
        let (main, zoom) = maps.as_mut().into_parts();
        blend(main, &mut self.planes, format, weight.main(), clear);
        if let Some((ph, bands)) = zoom.zip(self.zoom_planes_mut()) {
            blend(ph, bands, format, weight.zoom().unwrap_or_default(), clear);
        }
    }
}

fn blend(ph: &mut Phosphor, bands: &mut [DiffBand], format: CountFormat, weight: f64, clear: bool) {
    if clear {
        ph.clear();
    }
    if format == CountFormat::Wide {
        ph.set_format(format);
    }
    for band in bands {
        band.fold_picture([None, None], ph);
    }
    ph.add_weight(weight);
}
