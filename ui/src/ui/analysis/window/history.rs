// SPDX-License-Identifier: GPL-2.0-or-later
//! Pooling follows selected data and analyser settings, independently of acquisition epochs.

use super::*;

#[derive(PartialEq)]
struct Source {
    on: bool,
    scale: (usize, f32, f32),
    correction: (f32, f32, f32),
    input: (Coupling, usize, bool),
}

impl From<&Channel> for Source {
    fn from(c: &Channel) -> Self {
        Self {
            on: c.on,
            scale: (c.vscale, c.vfine, c.probe),
            correction: (c.bias, c.skew, c.offset),
            input: (c.coupling, c.bw, c.invert),
        }
    }
}

#[derive(PartialEq)]
pub(in crate::ui) struct Key {
    revision: u64,
    acquisition: AcqMode,
    channels: [Option<Source>; NCH],
    view: (AView, usize),
    spectrum: Option<SpectrumKey>,
    eye: Option<(WaveId, f64)>,
    power: Option<(WaveId, WaveId, PowerMode)>,
    bandwidth: Option<(usize, f64, usize)>,
    /// A new capture must compute Bandwidth even within the previous capture's throttle period.
    bandwidth_record: Option<(u64, usize, u32, usize)>,
    area: (i32, i32, i32, i32),
}

impl Key {
    pub(super) fn of(k: &super::Key, view: AView) -> Self {
        let channels = std::array::from_fn(|c| {
            let selected = match view {
                AView::Spectrum => WaveId::Ch(c) == k.spectrum.0,
                AView::Eye => WaveId::Ch(c) == k.eye.0,
                AView::Power => WaveId::Ch(c) == k.power.0 || WaveId::Ch(c) == k.power.1,
                AView::Step => c == k.bandwidth.0.min(NCH - 1),
                _ => false,
            };
            selected.then(|| Source::from(&k.channels[c]))
        });
        Self {
            revision: k.generation.1,
            acquisition: k.acquisition.0,
            channels,
            view: k.view,
            spectrum: (view == AView::Spectrum).then_some(k.spectrum),
            eye: (view == AView::Eye).then_some(k.eye),
            power: (view == AView::Power).then_some(k.power),
            bandwidth: (view == AView::Step).then_some(k.bandwidth),
            bandwidth_record: (view == AView::Step).then_some((
                k.generation.0.epoch(),
                k.acquisition.1,
                k.acquisition.2,
                k.acquisition.3,
            )),
            area: k.area,
        }
    }
}
