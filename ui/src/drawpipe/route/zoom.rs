// SPDX-License-Identifier: GPL-2.0-or-later

use crate::model::{tscales, Channel, Model};
use crate::picture::deposit::{kernel::Projection, standard};

/// Geometry of the detail map captured alongside the main worker view.
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct ZoomView {
    size: (usize, usize),
    factor: f32,
    center: f32,
}

impl ZoomView {
    pub(crate) fn new(size: (usize, usize), factor: f32, center: f32) -> Self {
        Self { size, factor, center }
    }

    pub(crate) fn size(self) -> (usize, usize) {
        self.size
    }

    #[cfg(any(target_os = "linux", test))]
    pub(crate) fn fixture_geometry(self) -> ((usize, usize), f32, f32) {
        (self.size, self.factor, self.center)
    }

    pub(crate) fn layout(self, main: (usize, usize)) -> oos_core::change::Layout {
        oos_core::change::Layout::new(main, self.size, (0, 0), None, 0, 0).with_vertical_zoom(self.factor, self.center)
    }

    pub(crate) fn transform(self, channel: &mut Channel) {
        let vpd = channel.volts_per_div();
        channel.vfine /= self.factor;
        channel.offset -= self.center * vpd;
    }

    pub(super) fn projection(self, model: &Model, dt: f64) -> Projection {
        let (w, h) = self.size;
        let tdiv = f64::from(tscales()[model.zscale]);
        let mut projection = standard::projection(model, w, h, dt / tdiv * w as f64 / f64::from(super::HDIV));
        projection.time_per_div = tdiv;
        projection.delay = f64::from(model.zdelay);
        for (channel, vertical) in model.ch.iter().zip(&mut projection.ch) {
            if let Some(vertical) = vertical {
                let mut channel = channel.clone();
                self.transform(&mut channel);
                vertical.volts_per_div = channel.volts_per_div();
                vertical.offset = channel.offset;
            }
        }
        projection
    }
}
