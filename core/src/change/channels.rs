// SPDX-License-Identifier: GPL-2.0-or-later

use super::{Classes as C, FieldDiff};
use crate::model::Model;

pub(super) fn channels_0(a: &Model, b: &Model, emit: &mut impl FnMut(FieldDiff)) {
    emit(FieldDiff::new(1, "active", a.active != b.active, C::NONE));
    emit(FieldDiff::new(59, "ch_label", a.ch_label != b.ch_label, C::NONE));
    emit(FieldDiff::new(67, "fine_v", a.fine_v != b.fine_v, C::NONE));
    emit(FieldDiff::new(70, "v_expand_gnd", a.v_expand_gnd != b.v_expand_gnd, C::NONE));
}

pub(super) fn refresh_0(a: &mut Model, b: &Model) {
    a.ch.clone_from(&b.ch);
    a.active.clone_from(&b.active);
    for (a, b) in a.ch_label.iter_mut().zip(&b.ch_label) {
        a.clone_from(b);
    }
    a.fine_v.clone_from(&b.fine_v);
    a.v_expand_gnd.clone_from(&b.v_expand_gnd);
}
