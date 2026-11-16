// SPDX-License-Identifier: GPL-2.0-or-later

//! Measurement options of the Measure menu.

use super::{Classes as C, FieldDiff};
use crate::model::Model;

pub(super) fn measure1(a: &Model, b: &Model, emit: &mut impl FnMut(FieldDiff)) {
    emit(FieldDiff::new(275, "meas_idle", a.meas_idle != b.meas_idle, C::STATS));
    emit(FieldDiff::new(276, "meas_out", a.meas_out != b.meas_out, C::STATS));
    emit(FieldDiff::new(277, "meas_n", a.meas_n != b.meas_n, C::STATS));
    emit(FieldDiff::new(278, "meas_data", a.meas_data != b.meas_data, C::STATS));
    emit(FieldDiff::new(279, "meas_lpct", a.meas_lpct != b.meas_lpct, C::STATS));
    emit(FieldDiff::new(280, "meas_hpct", a.meas_hpct != b.meas_hpct, C::STATS));
    emit(FieldDiff::new(281, "meas_full_record", a.meas_full_record != b.meas_full_record, C::STATS));
    emit(FieldDiff::new(282, "meas_tsetup", a.meas_tsetup != b.meas_tsetup, C::STATS));
    emit(FieldDiff::new(283, "meas_thold", a.meas_thold != b.meas_thold, C::STATS));
}

pub(super) fn refresh_measure1(a: &mut Model, b: &Model) {
    a.meas_idle.clone_from(&b.meas_idle);
    a.meas_out.clone_from(&b.meas_out);
    a.meas_n.clone_from(&b.meas_n);
    a.meas_data.clone_from(&b.meas_data);
    a.meas_lpct.clone_from(&b.meas_lpct);
    a.meas_hpct.clone_from(&b.meas_hpct);
    a.meas_full_record.clone_from(&b.meas_full_record);
    a.meas_tsetup.clone_from(&b.meas_tsetup);
    a.meas_thold.clone_from(&b.meas_thold);
}
