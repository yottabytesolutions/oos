// SPDX-License-Identifier: GPL-2.0-or-later

use crate::model::{Model, PowerMode};

#[derive(Clone, Debug, PartialEq)]
pub enum Analysis4 {
    SpecDist(bool),
    EyeSrc(usize),
    EyeRate(f64),
    PwV(usize),
    PwI(usize),
    PwMode(PowerMode),
    BodeStart(f64),
    BodeStop(f64),
    BodePoints(usize),
    BodeAmp(f64),
    BodeIn(usize),
    BodeOut(usize),
}

impl Analysis4 {
    pub(super) fn capture(index: usize, m: &Model) -> Option<Self> {
        Some(match index {
            140 => Self::SpecDist(m.spec_dist),
            141 => Self::EyeSrc(m.eye_src),
            142 => Self::EyeRate(m.eye_rate),
            143 => Self::PwV(m.pw_v),
            144 => Self::PwI(m.pw_i),
            145 => Self::PwMode(m.pw_mode),
            146 => Self::BodeStart(m.bode_start),
            147 => Self::BodeStop(m.bode_stop),
            148 => Self::BodePoints(m.bode_points),
            149 => Self::BodeAmp(m.bode_amp),
            150 => Self::BodeIn(m.bode_in),
            151 => Self::BodeOut(m.bode_out),
            _ => return None,
        })
    }
    pub(super) fn index(&self) -> usize {
        match self {
            Self::SpecDist(_) => 140,
            Self::EyeSrc(_) => 141,
            Self::EyeRate(_) => 142,
            Self::PwV(_) => 143,
            Self::PwI(_) => 144,
            Self::PwMode(_) => 145,
            Self::BodeStart(_) => 146,
            Self::BodeStop(_) => 147,
            Self::BodePoints(_) => 148,
            Self::BodeAmp(_) => 149,
            Self::BodeIn(_) => 150,
            Self::BodeOut(_) => 151,
        }
    }
    pub(super) fn apply(&self, m: &mut Model) {
        match self {
            Self::SpecDist(value) => m.spec_dist.clone_from(value),
            Self::EyeSrc(value) => m.eye_src.clone_from(value),
            Self::EyeRate(value) => m.eye_rate.clone_from(value),
            Self::PwV(value) => m.pw_v.clone_from(value),
            Self::PwI(value) => m.pw_i.clone_from(value),
            Self::PwMode(value) => m.pw_mode.clone_from(value),
            Self::BodeStart(value) => m.bode_start.clone_from(value),
            Self::BodeStop(value) => m.bode_stop.clone_from(value),
            Self::BodePoints(value) => m.bode_points.clone_from(value),
            Self::BodeAmp(value) => m.bode_amp.clone_from(value),
            Self::BodeIn(value) => m.bode_in.clone_from(value),
            Self::BodeOut(value) => m.bode_out.clone_from(value),
        }
    }
    pub(super) fn reuse(&mut self, m: &Model) {
        match self {
            Self::SpecDist(value) => value.clone_from(&m.spec_dist),
            Self::EyeSrc(value) => value.clone_from(&m.eye_src),
            Self::EyeRate(value) => value.clone_from(&m.eye_rate),
            Self::PwV(value) => value.clone_from(&m.pw_v),
            Self::PwI(value) => value.clone_from(&m.pw_i),
            Self::PwMode(value) => value.clone_from(&m.pw_mode),
            Self::BodeStart(value) => value.clone_from(&m.bode_start),
            Self::BodeStop(value) => value.clone_from(&m.bode_stop),
            Self::BodePoints(value) => value.clone_from(&m.bode_points),
            Self::BodeAmp(value) => value.clone_from(&m.bode_amp),
            Self::BodeIn(value) => value.clone_from(&m.bode_in),
            Self::BodeOut(value) => value.clone_from(&m.bode_out),
        }
    }
    pub(super) fn bytes(&self) -> usize {
        0
    }
}
