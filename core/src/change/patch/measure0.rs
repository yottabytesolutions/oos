// SPDX-License-Identifier: GPL-2.0-or-later

use crate::model::{CursorMode, Meas, Model};

#[derive(Clone, Debug, PartialEq)]
pub enum Measure0 {
    Meas(Vec<(Meas, usize)>),
    Meas2(Vec<(usize, usize, usize)>),
    DvmOn(bool),
    DvmSrc(usize),
    DvmMode(usize),
    CntOn(bool),
    CntSrc(usize),
    CntMode(usize),
    CntDigits(u32),
    Cursor(CursorMode),
    CurA(f32),
    CurB(f32),
}

impl Measure0 {
    pub(super) fn capture(index: usize, m: &Model) -> Option<Self> {
        Some(match index {
            50 => Self::Meas(m.meas.clone()),
            51 => Self::Meas2(m.meas2.clone()),
            52 => Self::DvmOn(m.dvm_on),
            53 => Self::DvmSrc(m.dvm_src),
            54 => Self::DvmMode(m.dvm_mode),
            55 => Self::CntOn(m.cnt_on),
            56 => Self::CntSrc(m.cnt_src),
            57 => Self::CntMode(m.cnt_mode),
            58 => Self::CntDigits(m.cnt_digits),
            60 => Self::Cursor(m.cursor),
            61 => Self::CurA(m.cur_a),
            62 => Self::CurB(m.cur_b),
            _ => return None,
        })
    }
    pub(super) fn index(&self) -> usize {
        match self {
            Self::Meas(_) => 50,
            Self::Meas2(_) => 51,
            Self::DvmOn(_) => 52,
            Self::DvmSrc(_) => 53,
            Self::DvmMode(_) => 54,
            Self::CntOn(_) => 55,
            Self::CntSrc(_) => 56,
            Self::CntMode(_) => 57,
            Self::CntDigits(_) => 58,
            Self::Cursor(_) => 60,
            Self::CurA(_) => 61,
            Self::CurB(_) => 62,
        }
    }
    pub(super) fn apply(&self, m: &mut Model) {
        match self {
            Self::Meas(value) => m.meas.clone_from(value),
            Self::Meas2(value) => m.meas2.clone_from(value),
            Self::DvmOn(value) => m.dvm_on.clone_from(value),
            Self::DvmSrc(value) => m.dvm_src.clone_from(value),
            Self::DvmMode(value) => m.dvm_mode.clone_from(value),
            Self::CntOn(value) => m.cnt_on.clone_from(value),
            Self::CntSrc(value) => m.cnt_src.clone_from(value),
            Self::CntMode(value) => m.cnt_mode.clone_from(value),
            Self::CntDigits(value) => m.cnt_digits.clone_from(value),
            Self::Cursor(value) => m.cursor.clone_from(value),
            Self::CurA(value) => m.cur_a.clone_from(value),
            Self::CurB(value) => m.cur_b.clone_from(value),
        }
    }
    pub(super) fn reuse(&mut self, m: &Model) {
        match self {
            Self::Meas(value) => value.clone_from(&m.meas),
            Self::Meas2(value) => value.clone_from(&m.meas2),
            Self::DvmOn(value) => value.clone_from(&m.dvm_on),
            Self::DvmSrc(value) => value.clone_from(&m.dvm_src),
            Self::DvmMode(value) => value.clone_from(&m.dvm_mode),
            Self::CntOn(value) => value.clone_from(&m.cnt_on),
            Self::CntSrc(value) => value.clone_from(&m.cnt_src),
            Self::CntMode(value) => value.clone_from(&m.cnt_mode),
            Self::CntDigits(value) => value.clone_from(&m.cnt_digits),
            Self::Cursor(value) => value.clone_from(&m.cursor),
            Self::CurA(value) => value.clone_from(&m.cur_a),
            Self::CurB(value) => value.clone_from(&m.cur_b),
        }
    }
    pub(super) fn bytes(&self) -> usize {
        match self {
            Self::Meas(value) => value.capacity() * std::mem::size_of_val(value.first().unwrap_or(&(Meas::Vavg, 0))),
            Self::Meas2(value) => value.capacity() * std::mem::size_of_val(value.first().unwrap_or(&(0, 0, 0))),
            _ => 0,
        }
    }
}
