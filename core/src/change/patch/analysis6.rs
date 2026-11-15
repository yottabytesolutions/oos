// SPDX-License-Identifier: GPL-2.0-or-later

use crate::model::Model;

#[derive(Clone, Debug, PartialEq)]
pub enum Analysis6 {
    BwBins(usize),
    MaskOn(bool),
    Zones([crate::zone::Zone; 2]),
    MaskSrc(usize),
    MaskX(f32),
    MaskY(f32),
    MaskStop(bool),
}

impl Analysis6 {
    pub(super) fn capture(index: usize, m: &Model) -> Option<Self> {
        Some(match index {
            167 => Self::BwBins(m.bw_bins),
            185 => Self::MaskOn(m.mask_on),
            186 => Self::Zones(m.zones.clone()),
            187 => Self::MaskSrc(m.mask_src),
            188 => Self::MaskX(m.mask_x),
            189 => Self::MaskY(m.mask_y),
            190 => Self::MaskStop(m.mask_stop),
            _ => return None,
        })
    }
    pub(super) fn index(&self) -> usize {
        match self {
            Self::BwBins(_) => 167,
            Self::MaskOn(_) => 185,
            Self::Zones(_) => 186,
            Self::MaskSrc(_) => 187,
            Self::MaskX(_) => 188,
            Self::MaskY(_) => 189,
            Self::MaskStop(_) => 190,
        }
    }
    pub(super) fn apply(&self, m: &mut Model) {
        match self {
            Self::BwBins(value) => m.bw_bins.clone_from(value),
            Self::MaskOn(value) => m.mask_on.clone_from(value),
            Self::Zones(value) => {
                for (a, b) in m.zones.iter_mut().zip(value) {
                    a.on = b.on;
                    a.src = b.src;
                    a.inside = b.inside;
                    a.pts.clone_from(&b.pts);
                }
            }
            Self::MaskSrc(value) => m.mask_src.clone_from(value),
            Self::MaskX(value) => m.mask_x.clone_from(value),
            Self::MaskY(value) => m.mask_y.clone_from(value),
            Self::MaskStop(value) => m.mask_stop.clone_from(value),
        }
    }
    pub(super) fn reuse(&mut self, m: &Model) {
        match self {
            Self::BwBins(value) => value.clone_from(&m.bw_bins),
            Self::MaskOn(value) => value.clone_from(&m.mask_on),
            Self::Zones(value) => {
                for (a, b) in value.iter_mut().zip(&m.zones) {
                    a.on = b.on;
                    a.src = b.src;
                    a.inside = b.inside;
                    a.pts.clone_from(&b.pts);
                }
            }
            Self::MaskSrc(value) => value.clone_from(&m.mask_src),
            Self::MaskX(value) => value.clone_from(&m.mask_x),
            Self::MaskY(value) => value.clone_from(&m.mask_y),
            Self::MaskStop(value) => value.clone_from(&m.mask_stop),
        }
    }
    pub(super) fn bytes(&self) -> usize {
        match self {
            Self::Zones(value) => value.iter().map(|z| z.pts.capacity() * std::mem::size_of::<(f64, f64)>()).sum(),
            _ => 0,
        }
    }
}
