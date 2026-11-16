// SPDX-License-Identifier: GPL-2.0-or-later

use crate::model::{Model, WaveId};

#[derive(Clone, Debug, PartialEq)]
pub enum Analysis4P1 {
    PwVin(Option<WaveId>),
    PwClk(Option<WaveId>),
    PwRef(Option<f64>),
    BodeMode(oos_dsp::bode::Mode),
    SpecAcprW(f64),
    SpecAcprS(f64),
}

impl Analysis4P1 {
    pub(super) fn capture(index: usize, m: &Model) -> Option<Self> {
        Some(match index {
            269 => Self::PwVin(m.pw_vin),
            270 => Self::PwClk(m.pw_clk),
            271 => Self::PwRef(m.pw_ref),
            272 => Self::BodeMode(m.bode_mode),
            273 => Self::SpecAcprW(m.spec_acpr_w),
            274 => Self::SpecAcprS(m.spec_acpr_s),
            _ => return None,
        })
    }
    pub(super) fn index(&self) -> usize {
        match self {
            Self::PwVin(_) => 269,
            Self::PwClk(_) => 270,
            Self::PwRef(_) => 271,
            Self::BodeMode(_) => 272,
            Self::SpecAcprW(_) => 273,
            Self::SpecAcprS(_) => 274,
        }
    }
    pub(super) fn apply(&self, m: &mut Model) {
        match self {
            Self::PwVin(value) => m.pw_vin.clone_from(value),
            Self::PwClk(value) => m.pw_clk.clone_from(value),
            Self::PwRef(value) => m.pw_ref.clone_from(value),
            Self::BodeMode(value) => m.bode_mode.clone_from(value),
            Self::SpecAcprW(value) => m.spec_acpr_w.clone_from(value),
            Self::SpecAcprS(value) => m.spec_acpr_s.clone_from(value),
        }
    }
    pub(super) fn reuse(&mut self, m: &Model) {
        match self {
            Self::PwVin(value) => value.clone_from(&m.pw_vin),
            Self::PwClk(value) => value.clone_from(&m.pw_clk),
            Self::PwRef(value) => value.clone_from(&m.pw_ref),
            Self::BodeMode(value) => value.clone_from(&m.bode_mode),
            Self::SpecAcprW(value) => value.clone_from(&m.spec_acpr_w),
            Self::SpecAcprS(value) => value.clone_from(&m.spec_acpr_s),
        }
    }
    pub(super) fn bytes(&self) -> usize {
        0
    }
}
