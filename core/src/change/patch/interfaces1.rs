// SPDX-License-Identifier: GPL-2.0-or-later

use crate::model::{Model, Quick};

#[derive(Clone, Debug, PartialEq)]
pub enum Interfaces1 {
    AfgMod(usize),
    AfgModRate(f64),
    AfgModDepth(f64),
    AfgFmDev(f64),
    AfgPmDev(f64),
    AfgModShape(usize),
    Quick(Quick),
}

impl Interfaces1 {
    pub(super) fn capture(index: usize, m: &Model) -> Option<Self> {
        Some(match index {
            179 => Self::AfgMod(m.afg_mod),
            180 => Self::AfgModRate(m.afg_mod_rate),
            181 => Self::AfgModDepth(m.afg_mod_depth),
            182 => Self::AfgFmDev(m.afg_fm_dev),
            183 => Self::AfgPmDev(m.afg_pm_dev),
            184 => Self::AfgModShape(m.afg_mod_shape),
            191 => Self::Quick(m.quick),
            _ => return None,
        })
    }
    pub(super) fn index(&self) -> usize {
        match self {
            Self::AfgMod(_) => 179,
            Self::AfgModRate(_) => 180,
            Self::AfgModDepth(_) => 181,
            Self::AfgFmDev(_) => 182,
            Self::AfgPmDev(_) => 183,
            Self::AfgModShape(_) => 184,
            Self::Quick(_) => 191,
        }
    }
    pub(super) fn apply(&self, m: &mut Model) {
        match self {
            Self::AfgMod(value) => m.afg_mod.clone_from(value),
            Self::AfgModRate(value) => m.afg_mod_rate.clone_from(value),
            Self::AfgModDepth(value) => m.afg_mod_depth.clone_from(value),
            Self::AfgFmDev(value) => m.afg_fm_dev.clone_from(value),
            Self::AfgPmDev(value) => m.afg_pm_dev.clone_from(value),
            Self::AfgModShape(value) => m.afg_mod_shape.clone_from(value),
            Self::Quick(value) => m.quick.clone_from(value),
        }
    }
    pub(super) fn reuse(&mut self, m: &Model) {
        match self {
            Self::AfgMod(value) => value.clone_from(&m.afg_mod),
            Self::AfgModRate(value) => value.clone_from(&m.afg_mod_rate),
            Self::AfgModDepth(value) => value.clone_from(&m.afg_mod_depth),
            Self::AfgFmDev(value) => value.clone_from(&m.afg_fm_dev),
            Self::AfgPmDev(value) => value.clone_from(&m.afg_pm_dev),
            Self::AfgModShape(value) => value.clone_from(&m.afg_mod_shape),
            Self::Quick(value) => value.clone_from(&m.quick),
        }
    }
    pub(super) fn bytes(&self) -> usize {
        0
    }
}
