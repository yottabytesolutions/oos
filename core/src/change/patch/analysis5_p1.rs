// SPDX-License-Identifier: GPL-2.0-or-later

use crate::model::{InvAngle, Model, WaveId};

#[derive(Clone, Debug, PartialEq)]
#[allow(
    clippy::enum_variant_names,
    reason = "one variant per Model field, named after the field and its shared prefix"
)]
pub enum Analysis5P1 {
    InvAng(InvAngle),
    InvPoles(u32),
    InvAn(WaveId),
    InvVolt(bool),
    InvTq(Option<WaveId>),
    InvEv(Option<WaveId>),
    InvEi(Option<WaveId>),
    InvScale(f64),
    InvDegv(f64),
    InvOff(f64),
    InvElec(bool),
    InvQa(u8),
}

impl Analysis5P1 {
    pub(super) fn capture(index: usize, m: &Model) -> Option<Self> {
        Some(match index {
            296 => Self::InvAng(m.inv_ang),
            297 => Self::InvPoles(m.inv_poles),
            298 => Self::InvAn(m.inv_an),
            299 => Self::InvVolt(m.inv_volt),
            300 => Self::InvTq(m.inv_tq),
            301 => Self::InvEv(m.inv_ev),
            302 => Self::InvEi(m.inv_ei),
            303 => Self::InvScale(m.inv_scale),
            304 => Self::InvDegv(m.inv_degv),
            305 => Self::InvOff(m.inv_off),
            306 => Self::InvElec(m.inv_elec),
            307 => Self::InvQa(m.inv_qa),
            _ => return None,
        })
    }
    pub(super) fn index(&self) -> usize {
        match self {
            Self::InvAng(_) => 296,
            Self::InvPoles(_) => 297,
            Self::InvAn(_) => 298,
            Self::InvVolt(_) => 299,
            Self::InvTq(_) => 300,
            Self::InvEv(_) => 301,
            Self::InvEi(_) => 302,
            Self::InvScale(_) => 303,
            Self::InvDegv(_) => 304,
            Self::InvOff(_) => 305,
            Self::InvElec(_) => 306,
            Self::InvQa(_) => 307,
        }
    }
    pub(super) fn apply(&self, m: &mut Model) {
        match self {
            Self::InvAng(value) => m.inv_ang.clone_from(value),
            Self::InvPoles(value) => m.inv_poles.clone_from(value),
            Self::InvAn(value) => m.inv_an.clone_from(value),
            Self::InvVolt(value) => m.inv_volt.clone_from(value),
            Self::InvTq(value) => m.inv_tq.clone_from(value),
            Self::InvEv(value) => m.inv_ev.clone_from(value),
            Self::InvEi(value) => m.inv_ei.clone_from(value),
            Self::InvScale(value) => m.inv_scale.clone_from(value),
            Self::InvDegv(value) => m.inv_degv.clone_from(value),
            Self::InvOff(value) => m.inv_off.clone_from(value),
            Self::InvElec(value) => m.inv_elec.clone_from(value),
            Self::InvQa(value) => m.inv_qa.clone_from(value),
        }
    }
    pub(super) fn reuse(&mut self, m: &Model) {
        match self {
            Self::InvAng(value) => value.clone_from(&m.inv_ang),
            Self::InvPoles(value) => value.clone_from(&m.inv_poles),
            Self::InvAn(value) => value.clone_from(&m.inv_an),
            Self::InvVolt(value) => value.clone_from(&m.inv_volt),
            Self::InvTq(value) => value.clone_from(&m.inv_tq),
            Self::InvEv(value) => value.clone_from(&m.inv_ev),
            Self::InvEi(value) => value.clone_from(&m.inv_ei),
            Self::InvScale(value) => value.clone_from(&m.inv_scale),
            Self::InvDegv(value) => value.clone_from(&m.inv_degv),
            Self::InvOff(value) => value.clone_from(&m.inv_off),
            Self::InvElec(value) => value.clone_from(&m.inv_elec),
            Self::InvQa(value) => value.clone_from(&m.inv_qa),
        }
    }
    pub(super) fn bytes(&self) -> usize {
        0
    }
}
