// SPDX-License-Identifier: GPL-2.0-or-later

use crate::model::{EyePlace, Model, WaveId};

#[derive(Clone, Debug, PartialEq)]
pub enum Analysis4P0 {
    EyeFromRecord(bool),
    EyePlace(EyePlace),
    EyeBer(f64),
    EyePnLo(f64),
    EyePnHi(f64),
    EyeNpj(bool),
    EyeB(Option<WaveId>),
    EyeRail(Option<WaveId>),
    EyeNotch(f64),
    EyeHp(bool),
    PwFline(f64),
    PwFsw(f64),
}

impl Analysis4P0 {
    pub(super) fn capture(index: usize, m: &Model) -> Option<Self> {
        Some(match index {
            257 => Self::EyeFromRecord(m.eye_from_record),
            258 => Self::EyePlace(m.eye_place),
            259 => Self::EyeBer(m.eye_ber),
            260 => Self::EyePnLo(m.eye_pn_lo),
            261 => Self::EyePnHi(m.eye_pn_hi),
            262 => Self::EyeNpj(m.eye_npj),
            263 => Self::EyeB(m.eye_b),
            264 => Self::EyeRail(m.eye_rail),
            265 => Self::EyeNotch(m.eye_notch),
            266 => Self::EyeHp(m.eye_hp),
            267 => Self::PwFline(m.pw_fline),
            268 => Self::PwFsw(m.pw_fsw),
            _ => return None,
        })
    }
    pub(super) fn index(&self) -> usize {
        match self {
            Self::EyeFromRecord(_) => 257,
            Self::EyePlace(_) => 258,
            Self::EyeBer(_) => 259,
            Self::EyePnLo(_) => 260,
            Self::EyePnHi(_) => 261,
            Self::EyeNpj(_) => 262,
            Self::EyeB(_) => 263,
            Self::EyeRail(_) => 264,
            Self::EyeNotch(_) => 265,
            Self::EyeHp(_) => 266,
            Self::PwFline(_) => 267,
            Self::PwFsw(_) => 268,
        }
    }
    pub(super) fn apply(&self, m: &mut Model) {
        match self {
            Self::EyeFromRecord(value) => m.eye_from_record.clone_from(value),
            Self::EyePlace(value) => m.eye_place.clone_from(value),
            Self::EyeBer(value) => m.eye_ber.clone_from(value),
            Self::EyePnLo(value) => m.eye_pn_lo.clone_from(value),
            Self::EyePnHi(value) => m.eye_pn_hi.clone_from(value),
            Self::EyeNpj(value) => m.eye_npj.clone_from(value),
            Self::EyeB(value) => m.eye_b.clone_from(value),
            Self::EyeRail(value) => m.eye_rail.clone_from(value),
            Self::EyeNotch(value) => m.eye_notch.clone_from(value),
            Self::EyeHp(value) => m.eye_hp.clone_from(value),
            Self::PwFline(value) => m.pw_fline.clone_from(value),
            Self::PwFsw(value) => m.pw_fsw.clone_from(value),
        }
    }
    pub(super) fn reuse(&mut self, m: &Model) {
        match self {
            Self::EyeFromRecord(value) => value.clone_from(&m.eye_from_record),
            Self::EyePlace(value) => value.clone_from(&m.eye_place),
            Self::EyeBer(value) => value.clone_from(&m.eye_ber),
            Self::EyePnLo(value) => value.clone_from(&m.eye_pn_lo),
            Self::EyePnHi(value) => value.clone_from(&m.eye_pn_hi),
            Self::EyeNpj(value) => value.clone_from(&m.eye_npj),
            Self::EyeB(value) => value.clone_from(&m.eye_b),
            Self::EyeRail(value) => value.clone_from(&m.eye_rail),
            Self::EyeNotch(value) => value.clone_from(&m.eye_notch),
            Self::EyeHp(value) => value.clone_from(&m.eye_hp),
            Self::PwFline(value) => value.clone_from(&m.pw_fline),
            Self::PwFsw(value) => value.clone_from(&m.pw_fsw),
        }
    }
    pub(super) fn bytes(&self) -> usize {
        0
    }
}
