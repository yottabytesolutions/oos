// SPDX-License-Identifier: GPL-2.0-or-later

use super::{Classes as C, FieldDiff};
use crate::model::Model;

pub(super) fn interfaces_0(a: &Model, b: &Model, emit: &mut impl FnMut(FieldDiff)) {
    emit(FieldDiff::new(66, "aux_out", a.aux_out != b.aux_out, C::ACQ | C::STATS));
    emit(FieldDiff::new(168, "gen_sel", a.gen_sel != b.gen_sel, C::ACQ | C::STATS));
    emit(FieldDiff::new(169, "gen_bank", a.gen_bank != b.gen_bank, C::ACQ | C::STATS));
    emit(FieldDiff::new(170, "afg_on", a.afg_on != b.afg_on, C::ACQ | C::STATS));
    emit(FieldDiff::new(171, "afg_arb", a.afg_arb != b.afg_arb, C::ACQ | C::STATS));
    emit(FieldDiff::new(172, "afg_arb_src", a.afg_arb_src != b.afg_arb_src, C::ACQ | C::STATS));
    emit(FieldDiff::new(173, "afg_freq", a.afg_freq != b.afg_freq, C::ACQ | C::STATS));
    emit(FieldDiff::new(174, "afg_ampl", a.afg_ampl != b.afg_ampl, C::ACQ | C::STATS));
    emit(FieldDiff::new(175, "afg_offset", a.afg_offset != b.afg_offset, C::ACQ | C::STATS));
    emit(FieldDiff::new(176, "afg_wave", a.afg_wave != b.afg_wave, C::ACQ | C::STATS));
    emit(FieldDiff::new(177, "afg_duty", a.afg_duty != b.afg_duty, C::ACQ | C::STATS));
    emit(FieldDiff::new(178, "afg_symmetry", a.afg_symmetry != b.afg_symmetry, C::ACQ | C::STATS));
    emit(FieldDiff::new(179, "afg_mod", a.afg_mod != b.afg_mod, C::ACQ | C::STATS));
    emit(FieldDiff::new(180, "afg_mod_rate", a.afg_mod_rate != b.afg_mod_rate, C::ACQ | C::STATS));
    emit(FieldDiff::new(181, "afg_mod_depth", a.afg_mod_depth != b.afg_mod_depth, C::ACQ | C::STATS));
    emit(FieldDiff::new(182, "afg_fm_dev", a.afg_fm_dev != b.afg_fm_dev, C::ACQ | C::STATS));
    emit(FieldDiff::new(183, "afg_pm_dev", a.afg_pm_dev != b.afg_pm_dev, C::ACQ | C::STATS));
    emit(FieldDiff::new(184, "afg_mod_shape", a.afg_mod_shape != b.afg_mod_shape, C::ACQ | C::STATS));
    emit(FieldDiff::new(191, "quick", a.quick != b.quick, C::NONE));
}

pub(super) fn refresh_0(a: &mut Model, b: &Model) {
    a.aux_out.clone_from(&b.aux_out);
    a.gen_sel.clone_from(&b.gen_sel);
    a.gen_bank.clone_from(&b.gen_bank);
    a.afg_on.clone_from(&b.afg_on);
    a.afg_arb.clone_from(&b.afg_arb);
    a.afg_arb_src.clone_from(&b.afg_arb_src);
    a.afg_freq.clone_from(&b.afg_freq);
    a.afg_ampl.clone_from(&b.afg_ampl);
    a.afg_offset.clone_from(&b.afg_offset);
    a.afg_wave.clone_from(&b.afg_wave);
    a.afg_duty.clone_from(&b.afg_duty);
    a.afg_symmetry.clone_from(&b.afg_symmetry);
    a.afg_mod.clone_from(&b.afg_mod);
    a.afg_mod_rate.clone_from(&b.afg_mod_rate);
    a.afg_mod_depth.clone_from(&b.afg_mod_depth);
    a.afg_fm_dev.clone_from(&b.afg_fm_dev);
    a.afg_pm_dev.clone_from(&b.afg_pm_dev);
    a.afg_mod_shape.clone_from(&b.afg_mod_shape);
    a.quick.clone_from(&b.quick);
}
