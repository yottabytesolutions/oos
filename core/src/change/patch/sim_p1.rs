// SPDX-License-Identifier: GPL-2.0-or-later

use crate::model::Model;

#[derive(Clone, Debug, PartialEq)]
pub enum SimP1 {
    SimSscDeviation(f64),
    SimSscHz(f64),
    SimRj(f64),
    SimPj(f64),
    SimPjhz(f64),
    SimDcdPp(f64),
    SimCorrRj(f64),
    SimCorrHz(f64),
}

impl SimP1 {
    pub(super) fn capture(index: usize, m: &Model) -> Option<Self> {
        Some(match index {
            334 => Self::SimSscDeviation(m.sim_ssc_deviation),
            335 => Self::SimSscHz(m.sim_ssc_hz),
            336 => Self::SimRj(m.sim_rj),
            337 => Self::SimPj(m.sim_pj),
            338 => Self::SimPjhz(m.sim_pjhz),
            339 => Self::SimDcdPp(m.sim_dcd_pp),
            340 => Self::SimCorrRj(m.sim_corr_rj),
            341 => Self::SimCorrHz(m.sim_corr_hz),
            _ => return None,
        })
    }
    pub(super) fn index(&self) -> usize {
        match self {
            Self::SimSscDeviation(_) => 334,
            Self::SimSscHz(_) => 335,
            Self::SimRj(_) => 336,
            Self::SimPj(_) => 337,
            Self::SimPjhz(_) => 338,
            Self::SimDcdPp(_) => 339,
            Self::SimCorrRj(_) => 340,
            Self::SimCorrHz(_) => 341,
        }
    }
    pub(super) fn apply(&self, m: &mut Model) {
        match self {
            Self::SimSscDeviation(value) => m.sim_ssc_deviation.clone_from(value),
            Self::SimSscHz(value) => m.sim_ssc_hz.clone_from(value),
            Self::SimRj(value) => m.sim_rj.clone_from(value),
            Self::SimPj(value) => m.sim_pj.clone_from(value),
            Self::SimPjhz(value) => m.sim_pjhz.clone_from(value),
            Self::SimDcdPp(value) => m.sim_dcd_pp.clone_from(value),
            Self::SimCorrRj(value) => m.sim_corr_rj.clone_from(value),
            Self::SimCorrHz(value) => m.sim_corr_hz.clone_from(value),
        }
    }
    pub(super) fn reuse(&mut self, m: &Model) {
        match self {
            Self::SimSscDeviation(value) => value.clone_from(&m.sim_ssc_deviation),
            Self::SimSscHz(value) => value.clone_from(&m.sim_ssc_hz),
            Self::SimRj(value) => value.clone_from(&m.sim_rj),
            Self::SimPj(value) => value.clone_from(&m.sim_pj),
            Self::SimPjhz(value) => value.clone_from(&m.sim_pjhz),
            Self::SimDcdPp(value) => value.clone_from(&m.sim_dcd_pp),
            Self::SimCorrRj(value) => value.clone_from(&m.sim_corr_rj),
            Self::SimCorrHz(value) => value.clone_from(&m.sim_corr_hz),
        }
    }
    pub(super) fn bytes(&self) -> usize {
        0
    }
}
