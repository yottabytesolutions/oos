// SPDX-License-Identifier: GPL-2.0-or-later

use crate::model::{DptFourth, Model, WaveId};

#[derive(Clone, Debug, PartialEq)]
pub enum Analysis5P2 {
    InvQb(u8),
    InvQi(Option<u8>),
    InvCpr(f64),
    InvSwap(bool),
    InvHa(u8),
    InvHb(u8),
    InvHc(u8),
    DptV(WaveId),
    DptI(WaveId),
    DptG(WaveId),
    Dpt4th(DptFourth),
    Dpt4ch(WaveId),
}

impl Analysis5P2 {
    pub(super) fn capture(index: usize, m: &Model) -> Option<Self> {
        Some(match index {
            308 => Self::InvQb(m.inv_qb),
            309 => Self::InvQi(m.inv_qi),
            310 => Self::InvCpr(m.inv_cpr),
            311 => Self::InvSwap(m.inv_swap),
            312 => Self::InvHa(m.inv_ha),
            313 => Self::InvHb(m.inv_hb),
            314 => Self::InvHc(m.inv_hc),
            315 => Self::DptV(m.dpt_v),
            316 => Self::DptI(m.dpt_i),
            317 => Self::DptG(m.dpt_g),
            318 => Self::Dpt4th(m.dpt_4th),
            319 => Self::Dpt4ch(m.dpt_4ch),
            _ => return None,
        })
    }
    pub(super) fn index(&self) -> usize {
        match self {
            Self::InvQb(_) => 308,
            Self::InvQi(_) => 309,
            Self::InvCpr(_) => 310,
            Self::InvSwap(_) => 311,
            Self::InvHa(_) => 312,
            Self::InvHb(_) => 313,
            Self::InvHc(_) => 314,
            Self::DptV(_) => 315,
            Self::DptI(_) => 316,
            Self::DptG(_) => 317,
            Self::Dpt4th(_) => 318,
            Self::Dpt4ch(_) => 319,
        }
    }
    pub(super) fn apply(&self, m: &mut Model) {
        match self {
            Self::InvQb(value) => m.inv_qb.clone_from(value),
            Self::InvQi(value) => m.inv_qi.clone_from(value),
            Self::InvCpr(value) => m.inv_cpr.clone_from(value),
            Self::InvSwap(value) => m.inv_swap.clone_from(value),
            Self::InvHa(value) => m.inv_ha.clone_from(value),
            Self::InvHb(value) => m.inv_hb.clone_from(value),
            Self::InvHc(value) => m.inv_hc.clone_from(value),
            Self::DptV(value) => m.dpt_v.clone_from(value),
            Self::DptI(value) => m.dpt_i.clone_from(value),
            Self::DptG(value) => m.dpt_g.clone_from(value),
            Self::Dpt4th(value) => m.dpt_4th.clone_from(value),
            Self::Dpt4ch(value) => m.dpt_4ch.clone_from(value),
        }
    }
    pub(super) fn reuse(&mut self, m: &Model) {
        match self {
            Self::InvQb(value) => value.clone_from(&m.inv_qb),
            Self::InvQi(value) => value.clone_from(&m.inv_qi),
            Self::InvCpr(value) => value.clone_from(&m.inv_cpr),
            Self::InvSwap(value) => value.clone_from(&m.inv_swap),
            Self::InvHa(value) => value.clone_from(&m.inv_ha),
            Self::InvHb(value) => value.clone_from(&m.inv_hb),
            Self::InvHc(value) => value.clone_from(&m.inv_hc),
            Self::DptV(value) => value.clone_from(&m.dpt_v),
            Self::DptI(value) => value.clone_from(&m.dpt_i),
            Self::DptG(value) => value.clone_from(&m.dpt_g),
            Self::Dpt4th(value) => value.clone_from(&m.dpt_4th),
            Self::Dpt4ch(value) => value.clone_from(&m.dpt_4ch),
        }
    }
    pub(super) fn bytes(&self) -> usize {
        0
    }
}
