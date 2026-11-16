// SPDX-License-Identifier: GPL-2.0-or-later

//! SIM generator: preset, slot and the knobs that override it. A change regenerates the SIM record (a new acquisition).

use super::{Classes as C, FieldDiff};
use crate::model::Model;

pub(super) fn sim(a: &Model, b: &Model, emit: &mut impl FnMut(FieldDiff)) {
    emit(FieldDiff::new(322, "sim_preset", a.sim_preset != b.sim_preset, C::ACQ | C::STATS));
    emit(FieldDiff::new(323, "sim_slot", a.sim_slot != b.sim_slot, C::ACQ | C::STATS));
    emit(FieldDiff::new(324, "sim_rate", a.sim_rate != b.sim_rate, C::ACQ | C::STATS));
    emit(FieldDiff::new(325, "sim_prbs", a.sim_prbs != b.sim_prbs, C::ACQ | C::STATS));
    emit(FieldDiff::new(326, "sim_vhigh", a.sim_vhigh != b.sim_vhigh, C::ACQ | C::STATS));
    emit(FieldDiff::new(327, "sim_vlow", a.sim_vlow != b.sim_vlow, C::ACQ | C::STATS));
    emit(FieldDiff::new(328, "sim_rise", a.sim_rise != b.sim_rise, C::ACQ | C::STATS));
    emit(FieldDiff::new(329, "sim_noise", a.sim_noise != b.sim_noise, C::ACQ | C::STATS));
    emit(FieldDiff::new(330, "sim_bw", a.sim_bw != b.sim_bw, C::ACQ | C::STATS));
    emit(FieldDiff::new(331, "sim_deemph", a.sim_deemph != b.sim_deemph, C::ACQ | C::STATS));
    emit(FieldDiff::new(332, "sim_ssc", a.sim_ssc != b.sim_ssc, C::ACQ | C::STATS));
    emit(FieldDiff::new(333, "sim_ssc_center", a.sim_ssc_center != b.sim_ssc_center, C::ACQ | C::STATS));
    emit(FieldDiff::new(334, "sim_ssc_deviation", a.sim_ssc_deviation != b.sim_ssc_deviation, C::ACQ | C::STATS));
    emit(FieldDiff::new(335, "sim_ssc_hz", a.sim_ssc_hz != b.sim_ssc_hz, C::ACQ | C::STATS));
    emit(FieldDiff::new(336, "sim_rj", a.sim_rj != b.sim_rj, C::ACQ | C::STATS));
    emit(FieldDiff::new(337, "sim_pj", a.sim_pj != b.sim_pj, C::ACQ | C::STATS));
    emit(FieldDiff::new(338, "sim_pjhz", a.sim_pjhz != b.sim_pjhz, C::ACQ | C::STATS));
    emit(FieldDiff::new(339, "sim_dcd_pp", a.sim_dcd_pp != b.sim_dcd_pp, C::ACQ | C::STATS));
    emit(FieldDiff::new(340, "sim_corr_rj", a.sim_corr_rj != b.sim_corr_rj, C::ACQ | C::STATS));
    emit(FieldDiff::new(341, "sim_corr_hz", a.sim_corr_hz != b.sim_corr_hz, C::ACQ | C::STATS));
}

pub(super) fn refresh_sim(a: &mut Model, b: &Model) {
    a.sim_preset.clone_from(&b.sim_preset);
    a.sim_slot.clone_from(&b.sim_slot);
    a.sim_rate.clone_from(&b.sim_rate);
    a.sim_prbs.clone_from(&b.sim_prbs);
    a.sim_vhigh.clone_from(&b.sim_vhigh);
    a.sim_vlow.clone_from(&b.sim_vlow);
    a.sim_rise.clone_from(&b.sim_rise);
    a.sim_noise.clone_from(&b.sim_noise);
    a.sim_bw.clone_from(&b.sim_bw);
    a.sim_deemph.clone_from(&b.sim_deemph);
    a.sim_ssc.clone_from(&b.sim_ssc);
    a.sim_ssc_center.clone_from(&b.sim_ssc_center);
    a.sim_ssc_deviation.clone_from(&b.sim_ssc_deviation);
    a.sim_ssc_hz.clone_from(&b.sim_ssc_hz);
    a.sim_rj.clone_from(&b.sim_rj);
    a.sim_pj.clone_from(&b.sim_pj);
    a.sim_pjhz.clone_from(&b.sim_pjhz);
    a.sim_dcd_pp.clone_from(&b.sim_dcd_pp);
    a.sim_corr_rj.clone_from(&b.sim_corr_rj);
    a.sim_corr_hz.clone_from(&b.sim_corr_hz);
}
