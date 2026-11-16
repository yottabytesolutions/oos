// SPDX-License-Identifier: GPL-2.0-or-later

use crate::model::{DptZoom, Model};

#[derive(Clone, Debug, PartialEq)]
pub enum Analysis5P3 {
    DptPct(f64),
    DptZoom(DptZoom),
}

impl Analysis5P3 {
    pub(super) fn capture(index: usize, m: &Model) -> Option<Self> {
        Some(match index {
            320 => Self::DptPct(m.dpt_pct),
            321 => Self::DptZoom(m.dpt_zoom),
            _ => return None,
        })
    }
    pub(super) fn index(&self) -> usize {
        match self {
            Self::DptPct(_) => 320,
            Self::DptZoom(_) => 321,
        }
    }
    pub(super) fn apply(&self, m: &mut Model) {
        match self {
            Self::DptPct(value) => m.dpt_pct.clone_from(value),
            Self::DptZoom(value) => m.dpt_zoom.clone_from(value),
        }
    }
    pub(super) fn reuse(&mut self, m: &Model) {
        match self {
            Self::DptPct(value) => value.clone_from(&m.dpt_pct),
            Self::DptZoom(value) => value.clone_from(&m.dpt_zoom),
        }
    }
    pub(super) fn bytes(&self) -> usize {
        0
    }
}
