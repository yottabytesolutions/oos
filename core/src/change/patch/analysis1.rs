// SPDX-License-Identifier: GPL-2.0-or-later

use crate::model::{DecCfg, Model, DECODERS};

#[derive(Clone, Debug, PartialEq)]
pub enum Analysis1 {
    Maths(Box<[crate::math::MathCh; crate::math::NMATH]>),
    Decs(Box<[Option<DecCfg>; DECODERS]>),
    DecSel(usize),
    DecOn(bool),
    DecSrc(usize),
    DecBaud(usize),
    DecCanrate(usize),
    DecCanh(bool),
    DecFdrate(usize),
    DecArate(usize),
    DecTick(usize),
    DecNibbles(u32),
}

impl Analysis1 {
    pub(super) fn capture(index: usize, m: &Model) -> Option<Self> {
        Some(match index {
            85 => Self::Maths(Box::new(m.maths)),
            86 => Self::Decs(Box::new(m.decs.clone())),
            87 => Self::DecSel(m.dec_sel),
            91 => Self::DecOn(m.dec_on),
            92 => Self::DecSrc(m.dec_src),
            93 => Self::DecBaud(m.dec_baud),
            94 => Self::DecCanrate(m.dec_canrate),
            95 => Self::DecCanh(m.dec_canh),
            96 => Self::DecFdrate(m.dec_fdrate),
            97 => Self::DecArate(m.dec_arate),
            98 => Self::DecTick(m.dec_tick),
            99 => Self::DecNibbles(m.dec_nibbles),
            _ => return None,
        })
    }
    pub(super) fn index(&self) -> usize {
        match self {
            Self::Maths(_) => 85,
            Self::Decs(_) => 86,
            Self::DecSel(_) => 87,
            Self::DecOn(_) => 91,
            Self::DecSrc(_) => 92,
            Self::DecBaud(_) => 93,
            Self::DecCanrate(_) => 94,
            Self::DecCanh(_) => 95,
            Self::DecFdrate(_) => 96,
            Self::DecArate(_) => 97,
            Self::DecTick(_) => 98,
            Self::DecNibbles(_) => 99,
        }
    }
    pub(super) fn apply(&self, m: &mut Model) {
        match self {
            Self::Maths(value) => m.maths.clone_from(value.as_ref()),
            Self::Decs(value) => m.decs.clone_from(value.as_ref()),
            Self::DecSel(value) => m.dec_sel.clone_from(value),
            Self::DecOn(value) => m.dec_on.clone_from(value),
            Self::DecSrc(value) => m.dec_src.clone_from(value),
            Self::DecBaud(value) => m.dec_baud.clone_from(value),
            Self::DecCanrate(value) => m.dec_canrate.clone_from(value),
            Self::DecCanh(value) => m.dec_canh.clone_from(value),
            Self::DecFdrate(value) => m.dec_fdrate.clone_from(value),
            Self::DecArate(value) => m.dec_arate.clone_from(value),
            Self::DecTick(value) => m.dec_tick.clone_from(value),
            Self::DecNibbles(value) => m.dec_nibbles.clone_from(value),
        }
    }
    pub(super) fn reuse(&mut self, m: &Model) {
        match self {
            Self::Maths(value) => value.as_mut().clone_from(&m.maths),
            Self::Decs(value) => value.as_mut().clone_from(&m.decs),
            Self::DecSel(value) => value.clone_from(&m.dec_sel),
            Self::DecOn(value) => value.clone_from(&m.dec_on),
            Self::DecSrc(value) => value.clone_from(&m.dec_src),
            Self::DecBaud(value) => value.clone_from(&m.dec_baud),
            Self::DecCanrate(value) => value.clone_from(&m.dec_canrate),
            Self::DecCanh(value) => value.clone_from(&m.dec_canh),
            Self::DecFdrate(value) => value.clone_from(&m.dec_fdrate),
            Self::DecArate(value) => value.clone_from(&m.dec_arate),
            Self::DecTick(value) => value.clone_from(&m.dec_tick),
            Self::DecNibbles(value) => value.clone_from(&m.dec_nibbles),
        }
    }
    pub(super) fn bytes(&self) -> usize {
        match self {
            Self::Maths(value) => std::mem::size_of_val(value.as_ref()),
            Self::Decs(value) => std::mem::size_of_val(value.as_ref()),
            _ => 0,
        }
    }
}
