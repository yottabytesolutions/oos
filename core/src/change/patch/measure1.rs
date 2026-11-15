// SPDX-License-Identifier: GPL-2.0-or-later

use crate::model::Model;

#[derive(Clone, Debug, PartialEq)]
pub enum Measure1 {
    CurC(f32),
    CurD(f32),
    MeasLabels(bool),
    TrendSel([usize; 2]),
    LogIvl(usize),
}

impl Measure1 {
    pub(super) fn capture(index: usize, m: &Model) -> Option<Self> {
        Some(match index {
            63 => Self::CurC(m.cur_c),
            64 => Self::CurD(m.cur_d),
            89 => Self::MeasLabels(m.meas_labels),
            90 => Self::TrendSel(m.trend_sel),
            79 => Self::LogIvl(m.log_ivl),
            _ => return None,
        })
    }
    pub(super) fn index(&self) -> usize {
        match self {
            Self::CurC(_) => 63,
            Self::CurD(_) => 64,
            Self::LogIvl(_) => 79,
            Self::MeasLabels(_) => 89,
            Self::TrendSel(_) => 90,
        }
    }
    pub(super) fn apply(&self, m: &mut Model) {
        match self {
            Self::CurC(value) => m.cur_c.clone_from(value),
            Self::CurD(value) => m.cur_d.clone_from(value),
            Self::MeasLabels(value) => m.meas_labels.clone_from(value),
            Self::TrendSel(value) => m.trend_sel.clone_from(value),
            Self::LogIvl(value) => m.log_ivl.clone_from(value),
        }
    }
    pub(super) fn reuse(&mut self, m: &Model) {
        match self {
            Self::CurC(value) => value.clone_from(&m.cur_c),
            Self::CurD(value) => value.clone_from(&m.cur_d),
            Self::MeasLabels(value) => value.clone_from(&m.meas_labels),
            Self::TrendSel(value) => value.clone_from(&m.trend_sel),
            Self::LogIvl(value) => value.clone_from(&m.log_ivl),
        }
    }
    pub(super) fn bytes(&self) -> usize {
        0
    }
}
