// SPDX-License-Identifier: GPL-2.0-or-later

use crate::model::{Bit, Model, Slope, TrigCoup, TrigMode, TrigType};

#[derive(Clone, Debug, PartialEq)]
pub enum Trigger0 {
    TrigSrc(usize),
    TrigLa(Option<u8>),
    PatLa([Bit; 16]),
    TrigLevel(f32),
    Slope(Slope),
    TrigMode(TrigMode),
    Holdoff(f32),
    TrigType(TrigType),
    TrigLevel2(f32),
    TrigNrej(bool),
    AutoNrej(bool),
    TrigCoup(TrigCoup),
}

impl Trigger0 {
    pub(super) fn capture(index: usize, m: &Model) -> Option<Self> {
        Some(match index {
            4 => Self::TrigSrc(m.trig_src),
            5 => Self::TrigLa(m.trig_la),
            6 => Self::PatLa(m.pat_la),
            7 => Self::TrigLevel(m.trig_level),
            8 => Self::Slope(m.slope),
            9 => Self::TrigMode(m.trig_mode),
            10 => Self::Holdoff(m.holdoff),
            11 => Self::TrigType(m.trig_type),
            12 => Self::TrigLevel2(m.trig_level2),
            13 => Self::TrigNrej(m.trig_nrej),
            14 => Self::AutoNrej(m.auto_nrej),
            15 => Self::TrigCoup(m.trig_coup),
            _ => return None,
        })
    }
    pub(super) fn index(&self) -> usize {
        match self {
            Self::TrigSrc(_) => 4,
            Self::TrigLa(_) => 5,
            Self::PatLa(_) => 6,
            Self::TrigLevel(_) => 7,
            Self::Slope(_) => 8,
            Self::TrigMode(_) => 9,
            Self::Holdoff(_) => 10,
            Self::TrigType(_) => 11,
            Self::TrigLevel2(_) => 12,
            Self::TrigNrej(_) => 13,
            Self::AutoNrej(_) => 14,
            Self::TrigCoup(_) => 15,
        }
    }
    pub(super) fn apply(&self, m: &mut Model) {
        match self {
            Self::TrigSrc(value) => m.trig_src.clone_from(value),
            Self::TrigLa(value) => m.trig_la.clone_from(value),
            Self::PatLa(value) => m.pat_la.clone_from(value),
            Self::TrigLevel(value) => m.trig_level.clone_from(value),
            Self::Slope(value) => m.slope.clone_from(value),
            Self::TrigMode(value) => m.trig_mode.clone_from(value),
            Self::Holdoff(value) => m.holdoff.clone_from(value),
            Self::TrigType(value) => m.trig_type.clone_from(value),
            Self::TrigLevel2(value) => m.trig_level2.clone_from(value),
            Self::TrigNrej(value) => m.trig_nrej.clone_from(value),
            Self::AutoNrej(value) => m.auto_nrej.clone_from(value),
            Self::TrigCoup(value) => m.trig_coup.clone_from(value),
        }
    }
    pub(super) fn reuse(&mut self, m: &Model) {
        match self {
            Self::TrigSrc(value) => value.clone_from(&m.trig_src),
            Self::TrigLa(value) => value.clone_from(&m.trig_la),
            Self::PatLa(value) => value.clone_from(&m.pat_la),
            Self::TrigLevel(value) => value.clone_from(&m.trig_level),
            Self::Slope(value) => value.clone_from(&m.slope),
            Self::TrigMode(value) => value.clone_from(&m.trig_mode),
            Self::Holdoff(value) => value.clone_from(&m.holdoff),
            Self::TrigType(value) => value.clone_from(&m.trig_type),
            Self::TrigLevel2(value) => value.clone_from(&m.trig_level2),
            Self::TrigNrej(value) => value.clone_from(&m.trig_nrej),
            Self::AutoNrej(value) => value.clone_from(&m.auto_nrej),
            Self::TrigCoup(value) => value.clone_from(&m.trig_coup),
        }
    }
    pub(super) fn bytes(&self) -> usize {
        0
    }
}
