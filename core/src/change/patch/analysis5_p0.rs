// SPDX-License-Identifier: GPL-2.0-or-later

use crate::model::{InvPage, MagPlot, Model, WaveId};

#[derive(Clone, Debug, PartialEq)]
pub enum Analysis5P0 {
    MagV(WaveId),
    MagI(WaveId),
    MagN(f64),
    MagAe(f64),
    MagLe(f64),
    MagRcu(f64),
    MagPlot(MagPlot),
    MagCur(usize),
    InvPage(InvPage),
    InvA(WaveId),
    InvB(WaveId),
    InvC(WaveId),
}

impl Analysis5P0 {
    pub(super) fn capture(index: usize, m: &Model) -> Option<Self> {
        Some(match index {
            284 => Self::MagV(m.mag_v),
            285 => Self::MagI(m.mag_i),
            286 => Self::MagN(m.mag_n),
            287 => Self::MagAe(m.mag_ae),
            288 => Self::MagLe(m.mag_le),
            289 => Self::MagRcu(m.mag_rcu),
            290 => Self::MagPlot(m.mag_plot),
            291 => Self::MagCur(m.mag_cur),
            292 => Self::InvPage(m.inv_page),
            293 => Self::InvA(m.inv_a),
            294 => Self::InvB(m.inv_b),
            295 => Self::InvC(m.inv_c),
            _ => return None,
        })
    }
    pub(super) fn index(&self) -> usize {
        match self {
            Self::MagV(_) => 284,
            Self::MagI(_) => 285,
            Self::MagN(_) => 286,
            Self::MagAe(_) => 287,
            Self::MagLe(_) => 288,
            Self::MagRcu(_) => 289,
            Self::MagPlot(_) => 290,
            Self::MagCur(_) => 291,
            Self::InvPage(_) => 292,
            Self::InvA(_) => 293,
            Self::InvB(_) => 294,
            Self::InvC(_) => 295,
        }
    }
    pub(super) fn apply(&self, m: &mut Model) {
        match self {
            Self::MagV(value) => m.mag_v.clone_from(value),
            Self::MagI(value) => m.mag_i.clone_from(value),
            Self::MagN(value) => m.mag_n.clone_from(value),
            Self::MagAe(value) => m.mag_ae.clone_from(value),
            Self::MagLe(value) => m.mag_le.clone_from(value),
            Self::MagRcu(value) => m.mag_rcu.clone_from(value),
            Self::MagPlot(value) => m.mag_plot.clone_from(value),
            Self::MagCur(value) => m.mag_cur.clone_from(value),
            Self::InvPage(value) => m.inv_page.clone_from(value),
            Self::InvA(value) => m.inv_a.clone_from(value),
            Self::InvB(value) => m.inv_b.clone_from(value),
            Self::InvC(value) => m.inv_c.clone_from(value),
        }
    }
    pub(super) fn reuse(&mut self, m: &Model) {
        match self {
            Self::MagV(value) => value.clone_from(&m.mag_v),
            Self::MagI(value) => value.clone_from(&m.mag_i),
            Self::MagN(value) => value.clone_from(&m.mag_n),
            Self::MagAe(value) => value.clone_from(&m.mag_ae),
            Self::MagLe(value) => value.clone_from(&m.mag_le),
            Self::MagRcu(value) => value.clone_from(&m.mag_rcu),
            Self::MagPlot(value) => value.clone_from(&m.mag_plot),
            Self::MagCur(value) => value.clone_from(&m.mag_cur),
            Self::InvPage(value) => value.clone_from(&m.inv_page),
            Self::InvA(value) => value.clone_from(&m.inv_a),
            Self::InvB(value) => value.clone_from(&m.inv_b),
            Self::InvC(value) => value.clone_from(&m.inv_c),
        }
    }
    pub(super) fn bytes(&self) -> usize {
        0
    }
}
