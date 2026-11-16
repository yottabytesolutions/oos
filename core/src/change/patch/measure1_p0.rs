// SPDX-License-Identifier: GPL-2.0-or-later

use crate::model::Model;

#[derive(Clone, Debug, PartialEq)]
pub enum Measure1P0 {
    MeasIdle(f64),
    MeasOut(oos_dsp::measure::Outside),
    MeasN(u32),
    MeasData(bool),
    MeasLpct(f64),
    MeasHpct(f64),
    MeasFullRecord(bool),
    MeasTsetup(f64),
    MeasThold(f64),
}

impl Measure1P0 {
    pub(super) fn capture(index: usize, m: &Model) -> Option<Self> {
        Some(match index {
            275 => Self::MeasIdle(m.meas_idle),
            276 => Self::MeasOut(m.meas_out),
            277 => Self::MeasN(m.meas_n),
            278 => Self::MeasData(m.meas_data),
            279 => Self::MeasLpct(m.meas_lpct),
            280 => Self::MeasHpct(m.meas_hpct),
            281 => Self::MeasFullRecord(m.meas_full_record),
            282 => Self::MeasTsetup(m.meas_tsetup),
            283 => Self::MeasThold(m.meas_thold),
            _ => return None,
        })
    }
    pub(super) fn index(&self) -> usize {
        match self {
            Self::MeasIdle(_) => 275,
            Self::MeasOut(_) => 276,
            Self::MeasN(_) => 277,
            Self::MeasData(_) => 278,
            Self::MeasLpct(_) => 279,
            Self::MeasHpct(_) => 280,
            Self::MeasFullRecord(_) => 281,
            Self::MeasTsetup(_) => 282,
            Self::MeasThold(_) => 283,
        }
    }
    pub(super) fn apply(&self, m: &mut Model) {
        match self {
            Self::MeasIdle(value) => m.meas_idle.clone_from(value),
            Self::MeasOut(value) => m.meas_out.clone_from(value),
            Self::MeasN(value) => m.meas_n.clone_from(value),
            Self::MeasData(value) => m.meas_data.clone_from(value),
            Self::MeasLpct(value) => m.meas_lpct.clone_from(value),
            Self::MeasHpct(value) => m.meas_hpct.clone_from(value),
            Self::MeasFullRecord(value) => m.meas_full_record.clone_from(value),
            Self::MeasTsetup(value) => m.meas_tsetup.clone_from(value),
            Self::MeasThold(value) => m.meas_thold.clone_from(value),
        }
    }
    pub(super) fn reuse(&mut self, m: &Model) {
        match self {
            Self::MeasIdle(value) => value.clone_from(&m.meas_idle),
            Self::MeasOut(value) => value.clone_from(&m.meas_out),
            Self::MeasN(value) => value.clone_from(&m.meas_n),
            Self::MeasData(value) => value.clone_from(&m.meas_data),
            Self::MeasLpct(value) => value.clone_from(&m.meas_lpct),
            Self::MeasHpct(value) => value.clone_from(&m.meas_hpct),
            Self::MeasFullRecord(value) => value.clone_from(&m.meas_full_record),
            Self::MeasTsetup(value) => value.clone_from(&m.meas_tsetup),
            Self::MeasThold(value) => value.clone_from(&m.meas_thold),
        }
    }
    pub(super) fn bytes(&self) -> usize {
        0
    }
}
