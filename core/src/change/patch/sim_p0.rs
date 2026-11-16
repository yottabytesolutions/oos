// SPDX-License-Identifier: GPL-2.0-or-later

use crate::model::Model;

#[derive(Clone, Debug, PartialEq)]
#[allow(
    clippy::enum_variant_names,
    reason = "one variant per Model field, named after the field and its shared prefix"
)]
pub enum SimP0 {
    SimPreset(usize),
    SimSlot(usize),
    SimRate(f64),
    SimPrbs(u8),
    SimVhigh(f64),
    SimVlow(f64),
    SimRise(f64),
    SimNoise(f64),
    SimBw(f64),
    SimDeemph(f64),
    SimSsc(bool),
    SimSscCenter(bool),
}

impl SimP0 {
    pub(super) fn capture(index: usize, m: &Model) -> Option<Self> {
        Some(match index {
            322 => Self::SimPreset(m.sim_preset),
            323 => Self::SimSlot(m.sim_slot),
            324 => Self::SimRate(m.sim_rate),
            325 => Self::SimPrbs(m.sim_prbs),
            326 => Self::SimVhigh(m.sim_vhigh),
            327 => Self::SimVlow(m.sim_vlow),
            328 => Self::SimRise(m.sim_rise),
            329 => Self::SimNoise(m.sim_noise),
            330 => Self::SimBw(m.sim_bw),
            331 => Self::SimDeemph(m.sim_deemph),
            332 => Self::SimSsc(m.sim_ssc),
            333 => Self::SimSscCenter(m.sim_ssc_center),
            _ => return None,
        })
    }
    pub(super) fn index(&self) -> usize {
        match self {
            Self::SimPreset(_) => 322,
            Self::SimSlot(_) => 323,
            Self::SimRate(_) => 324,
            Self::SimPrbs(_) => 325,
            Self::SimVhigh(_) => 326,
            Self::SimVlow(_) => 327,
            Self::SimRise(_) => 328,
            Self::SimNoise(_) => 329,
            Self::SimBw(_) => 330,
            Self::SimDeemph(_) => 331,
            Self::SimSsc(_) => 332,
            Self::SimSscCenter(_) => 333,
        }
    }
    pub(super) fn apply(&self, m: &mut Model) {
        match self {
            Self::SimPreset(value) => m.sim_preset.clone_from(value),
            Self::SimSlot(value) => m.sim_slot.clone_from(value),
            Self::SimRate(value) => m.sim_rate.clone_from(value),
            Self::SimPrbs(value) => m.sim_prbs.clone_from(value),
            Self::SimVhigh(value) => m.sim_vhigh.clone_from(value),
            Self::SimVlow(value) => m.sim_vlow.clone_from(value),
            Self::SimRise(value) => m.sim_rise.clone_from(value),
            Self::SimNoise(value) => m.sim_noise.clone_from(value),
            Self::SimBw(value) => m.sim_bw.clone_from(value),
            Self::SimDeemph(value) => m.sim_deemph.clone_from(value),
            Self::SimSsc(value) => m.sim_ssc.clone_from(value),
            Self::SimSscCenter(value) => m.sim_ssc_center.clone_from(value),
        }
    }
    pub(super) fn reuse(&mut self, m: &Model) {
        match self {
            Self::SimPreset(value) => value.clone_from(&m.sim_preset),
            Self::SimSlot(value) => value.clone_from(&m.sim_slot),
            Self::SimRate(value) => value.clone_from(&m.sim_rate),
            Self::SimPrbs(value) => value.clone_from(&m.sim_prbs),
            Self::SimVhigh(value) => value.clone_from(&m.sim_vhigh),
            Self::SimVlow(value) => value.clone_from(&m.sim_vlow),
            Self::SimRise(value) => value.clone_from(&m.sim_rise),
            Self::SimNoise(value) => value.clone_from(&m.sim_noise),
            Self::SimBw(value) => value.clone_from(&m.sim_bw),
            Self::SimDeemph(value) => value.clone_from(&m.sim_deemph),
            Self::SimSsc(value) => value.clone_from(&m.sim_ssc),
            Self::SimSscCenter(value) => value.clone_from(&m.sim_ssc_center),
        }
    }
    pub(super) fn bytes(&self) -> usize {
        0
    }
}
