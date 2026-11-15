// SPDX-License-Identifier: GPL-2.0-or-later

use super::{Classes as C, FieldDiff};
use crate::model::Model;

pub(super) fn measure_0(a: &Model, b: &Model, emit: &mut impl FnMut(FieldDiff)) {
    emit(FieldDiff::new(50, "meas", a.meas != b.meas, C::STATS));
    emit(FieldDiff::new(51, "meas2", a.meas2 != b.meas2, C::STATS));
    emit(FieldDiff::new(52, "dvm_on", a.dvm_on != b.dvm_on, C::ACQ | C::STATS));
    emit(FieldDiff::new(53, "dvm_src", a.dvm_src != b.dvm_src, C::ACQ | C::STATS));
    emit(FieldDiff::new(54, "dvm_mode", a.dvm_mode != b.dvm_mode, C::ACQ | C::STATS));
    emit(FieldDiff::new(55, "cnt_on", a.cnt_on != b.cnt_on, C::ACQ | C::STATS));
    emit(FieldDiff::new(56, "cnt_src", a.cnt_src != b.cnt_src, C::ACQ | C::STATS));
    emit(FieldDiff::new(57, "cnt_mode", a.cnt_mode != b.cnt_mode, C::ACQ | C::STATS));
    emit(FieldDiff::new(58, "cnt_digits", a.cnt_digits != b.cnt_digits, C::ACQ | C::STATS));
    emit(FieldDiff::new(60, "cursor", a.cursor != b.cursor, C::NONE));
    emit(FieldDiff::new(61, "cur_a", a.cur_a != b.cur_a, C::NONE));
    emit(FieldDiff::new(62, "cur_b", a.cur_b != b.cur_b, C::NONE));
    emit(FieldDiff::new(63, "cur_c", a.cur_c != b.cur_c, C::NONE));
    emit(FieldDiff::new(64, "cur_d", a.cur_d != b.cur_d, C::NONE));
    emit(FieldDiff::new(89, "meas_labels", a.meas_labels != b.meas_labels, C::NONE));
    emit(FieldDiff::new(90, "trend_sel", a.trend_sel != b.trend_sel, C::VIEW));
    emit(FieldDiff::new(79, "log_ivl", a.log_ivl != b.log_ivl, C::NONE));
}

pub(super) fn refresh_0(a: &mut Model, b: &Model) {
    a.meas.clone_from(&b.meas);
    a.meas2.clone_from(&b.meas2);
    a.dvm_on.clone_from(&b.dvm_on);
    a.dvm_src.clone_from(&b.dvm_src);
    a.dvm_mode.clone_from(&b.dvm_mode);
    a.cnt_on.clone_from(&b.cnt_on);
    a.cnt_src.clone_from(&b.cnt_src);
    a.cnt_mode.clone_from(&b.cnt_mode);
    a.cnt_digits.clone_from(&b.cnt_digits);
    a.cursor.clone_from(&b.cursor);
    a.cur_a.clone_from(&b.cur_a);
    a.cur_b.clone_from(&b.cur_b);
    a.cur_c.clone_from(&b.cur_c);
    a.cur_d.clone_from(&b.cur_d);
    a.meas_labels.clone_from(&b.meas_labels);
    a.trend_sel.clone_from(&b.trend_sel);
    a.log_ivl.clone_from(&b.log_ivl);
}
