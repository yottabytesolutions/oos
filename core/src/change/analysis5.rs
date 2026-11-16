// SPDX-License-Identifier: GPL-2.0-or-later

//! Magnetic, inverter and double-pulse analysers.

use super::{Classes as C, FieldDiff};
use crate::model::Model;

pub(super) fn analysis5(a: &Model, b: &Model, emit: &mut impl FnMut(FieldDiff)) {
    emit(FieldDiff::new(284, "mag_v", a.mag_v != b.mag_v, C::VIEW));
    emit(FieldDiff::new(285, "mag_i", a.mag_i != b.mag_i, C::VIEW));
    emit(FieldDiff::new(286, "mag_n", a.mag_n != b.mag_n, C::VIEW));
    emit(FieldDiff::new(287, "mag_ae", a.mag_ae != b.mag_ae, C::VIEW));
    emit(FieldDiff::new(288, "mag_le", a.mag_le != b.mag_le, C::VIEW));
    emit(FieldDiff::new(289, "mag_rcu", a.mag_rcu != b.mag_rcu, C::VIEW));
    emit(FieldDiff::new(290, "mag_plot", a.mag_plot != b.mag_plot, C::VIEW));
    emit(FieldDiff::new(291, "mag_cur", a.mag_cur != b.mag_cur, C::NONE));
    emit(FieldDiff::new(292, "inv_page", a.inv_page != b.inv_page, C::VIEW));
    emit(FieldDiff::new(293, "inv_a", a.inv_a != b.inv_a, C::VIEW));
    emit(FieldDiff::new(294, "inv_b", a.inv_b != b.inv_b, C::VIEW));
    emit(FieldDiff::new(295, "inv_c", a.inv_c != b.inv_c, C::VIEW));
    emit(FieldDiff::new(296, "inv_ang", a.inv_ang != b.inv_ang, C::VIEW));
    emit(FieldDiff::new(297, "inv_poles", a.inv_poles != b.inv_poles, C::VIEW));
    emit(FieldDiff::new(298, "inv_an", a.inv_an != b.inv_an, C::VIEW));
    emit(FieldDiff::new(299, "inv_volt", a.inv_volt != b.inv_volt, C::VIEW));
    emit(FieldDiff::new(300, "inv_tq", a.inv_tq != b.inv_tq, C::VIEW));
    emit(FieldDiff::new(301, "inv_ev", a.inv_ev != b.inv_ev, C::VIEW));
    emit(FieldDiff::new(302, "inv_ei", a.inv_ei != b.inv_ei, C::VIEW));
    emit(FieldDiff::new(303, "inv_scale", a.inv_scale != b.inv_scale, C::VIEW));
    emit(FieldDiff::new(304, "inv_degv", a.inv_degv != b.inv_degv, C::VIEW));
    emit(FieldDiff::new(305, "inv_off", a.inv_off != b.inv_off, C::VIEW));
    emit(FieldDiff::new(306, "inv_elec", a.inv_elec != b.inv_elec, C::VIEW));
    emit(FieldDiff::new(307, "inv_qa", a.inv_qa != b.inv_qa, C::VIEW));
    emit(FieldDiff::new(308, "inv_qb", a.inv_qb != b.inv_qb, C::VIEW));
    emit(FieldDiff::new(309, "inv_qi", a.inv_qi != b.inv_qi, C::VIEW));
    emit(FieldDiff::new(310, "inv_cpr", a.inv_cpr != b.inv_cpr, C::VIEW));
    emit(FieldDiff::new(311, "inv_swap", a.inv_swap != b.inv_swap, C::VIEW));
    emit(FieldDiff::new(312, "inv_ha", a.inv_ha != b.inv_ha, C::VIEW));
    emit(FieldDiff::new(313, "inv_hb", a.inv_hb != b.inv_hb, C::VIEW));
    emit(FieldDiff::new(314, "inv_hc", a.inv_hc != b.inv_hc, C::VIEW));
    emit(FieldDiff::new(315, "dpt_v", a.dpt_v != b.dpt_v, C::VIEW));
    emit(FieldDiff::new(316, "dpt_i", a.dpt_i != b.dpt_i, C::VIEW));
    emit(FieldDiff::new(317, "dpt_g", a.dpt_g != b.dpt_g, C::VIEW));
    emit(FieldDiff::new(318, "dpt_4th", a.dpt_4th != b.dpt_4th, C::VIEW));
    emit(FieldDiff::new(319, "dpt_4ch", a.dpt_4ch != b.dpt_4ch, C::VIEW));
    emit(FieldDiff::new(320, "dpt_pct", a.dpt_pct != b.dpt_pct, C::VIEW));
    emit(FieldDiff::new(321, "dpt_zoom", a.dpt_zoom != b.dpt_zoom, C::VIEW));
}

pub(super) fn refresh_analysis5(a: &mut Model, b: &Model) {
    a.mag_v.clone_from(&b.mag_v);
    a.mag_i.clone_from(&b.mag_i);
    a.mag_n.clone_from(&b.mag_n);
    a.mag_ae.clone_from(&b.mag_ae);
    a.mag_le.clone_from(&b.mag_le);
    a.mag_rcu.clone_from(&b.mag_rcu);
    a.mag_plot.clone_from(&b.mag_plot);
    a.mag_cur.clone_from(&b.mag_cur);
    a.inv_page.clone_from(&b.inv_page);
    a.inv_a.clone_from(&b.inv_a);
    a.inv_b.clone_from(&b.inv_b);
    a.inv_c.clone_from(&b.inv_c);
    a.inv_ang.clone_from(&b.inv_ang);
    a.inv_poles.clone_from(&b.inv_poles);
    a.inv_an.clone_from(&b.inv_an);
    a.inv_volt.clone_from(&b.inv_volt);
    a.inv_tq.clone_from(&b.inv_tq);
    a.inv_ev.clone_from(&b.inv_ev);
    a.inv_ei.clone_from(&b.inv_ei);
    a.inv_scale.clone_from(&b.inv_scale);
    a.inv_degv.clone_from(&b.inv_degv);
    a.inv_off.clone_from(&b.inv_off);
    a.inv_elec.clone_from(&b.inv_elec);
    a.inv_qa.clone_from(&b.inv_qa);
    a.inv_qb.clone_from(&b.inv_qb);
    a.inv_qi.clone_from(&b.inv_qi);
    a.inv_cpr.clone_from(&b.inv_cpr);
    a.inv_swap.clone_from(&b.inv_swap);
    a.inv_ha.clone_from(&b.inv_ha);
    a.inv_hb.clone_from(&b.inv_hb);
    a.inv_hc.clone_from(&b.inv_hc);
    a.dpt_v.clone_from(&b.dpt_v);
    a.dpt_i.clone_from(&b.dpt_i);
    a.dpt_g.clone_from(&b.dpt_g);
    a.dpt_4th.clone_from(&b.dpt_4th);
    a.dpt_4ch.clone_from(&b.dpt_4ch);
    a.dpt_pct.clone_from(&b.dpt_pct);
    a.dpt_zoom.clone_from(&b.dpt_zoom);
}
