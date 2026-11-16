// SPDX-License-Identifier: GPL-2.0-or-later

//! Eye, power, Bode mode and ACPR settings.

use super::{Classes as C, FieldDiff};
use crate::model::Model;

pub(super) fn analysis4(a: &Model, b: &Model, emit: &mut impl FnMut(FieldDiff)) {
    emit(FieldDiff::new(257, "eye_from_record", a.eye_from_record != b.eye_from_record, C::VIEW));
    emit(FieldDiff::new(258, "eye_place", a.eye_place != b.eye_place, C::VIEW));
    emit(FieldDiff::new(259, "eye_ber", a.eye_ber != b.eye_ber, C::VIEW));
    emit(FieldDiff::new(260, "eye_pn_lo", a.eye_pn_lo != b.eye_pn_lo, C::VIEW));
    emit(FieldDiff::new(261, "eye_pn_hi", a.eye_pn_hi != b.eye_pn_hi, C::VIEW));
    emit(FieldDiff::new(262, "eye_npj", a.eye_npj != b.eye_npj, C::VIEW));
    emit(FieldDiff::new(263, "eye_b", a.eye_b != b.eye_b, C::VIEW));
    emit(FieldDiff::new(264, "eye_rail", a.eye_rail != b.eye_rail, C::VIEW));
    emit(FieldDiff::new(265, "eye_notch", a.eye_notch != b.eye_notch, C::VIEW));
    emit(FieldDiff::new(266, "eye_hp", a.eye_hp != b.eye_hp, C::VIEW));
    emit(FieldDiff::new(267, "pw_fline", a.pw_fline != b.pw_fline, C::VIEW));
    emit(FieldDiff::new(268, "pw_fsw", a.pw_fsw != b.pw_fsw, C::VIEW));
    emit(FieldDiff::new(269, "pw_vin", a.pw_vin != b.pw_vin, C::VIEW));
    emit(FieldDiff::new(270, "pw_clk", a.pw_clk != b.pw_clk, C::VIEW));
    emit(FieldDiff::new(271, "pw_ref", a.pw_ref != b.pw_ref, C::VIEW));
    emit(FieldDiff::new(272, "bode_mode", a.bode_mode != b.bode_mode, C::STATS | C::VIEW));
    emit(FieldDiff::new(273, "spec_acpr_w", a.spec_acpr_w != b.spec_acpr_w, C::MATH | C::VIEW));
    emit(FieldDiff::new(274, "spec_acpr_s", a.spec_acpr_s != b.spec_acpr_s, C::MATH | C::VIEW));
}

pub(super) fn refresh_analysis4(a: &mut Model, b: &Model) {
    a.eye_from_record.clone_from(&b.eye_from_record);
    a.eye_place.clone_from(&b.eye_place);
    a.eye_ber.clone_from(&b.eye_ber);
    a.eye_pn_lo.clone_from(&b.eye_pn_lo);
    a.eye_pn_hi.clone_from(&b.eye_pn_hi);
    a.eye_npj.clone_from(&b.eye_npj);
    a.eye_b.clone_from(&b.eye_b);
    a.eye_rail.clone_from(&b.eye_rail);
    a.eye_notch.clone_from(&b.eye_notch);
    a.eye_hp.clone_from(&b.eye_hp);
    a.pw_fline.clone_from(&b.pw_fline);
    a.pw_fsw.clone_from(&b.pw_fsw);
    a.pw_vin.clone_from(&b.pw_vin);
    a.pw_clk.clone_from(&b.pw_clk);
    a.pw_ref.clone_from(&b.pw_ref);
    a.bode_mode.clone_from(&b.bode_mode);
    a.spec_acpr_w.clone_from(&b.spec_acpr_w);
    a.spec_acpr_s.clone_from(&b.spec_acpr_s);
}
