// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
pub(super) const CASES: &[Case] = &[
    (
        "mag_v",
        |m| {
            m.mag_v = WaveId::Ref(1);
        },
        C::VIEW,
    ),
    (
        "mag_i",
        |m| {
            m.mag_i = WaveId::Ref(1);
        },
        C::VIEW,
    ),
    (
        "mag_n",
        |m| {
            m.mag_n += 1.0;
        },
        C::VIEW,
    ),
    (
        "mag_ae",
        |m| {
            m.mag_ae += 1.0;
        },
        C::VIEW,
    ),
    (
        "mag_le",
        |m| {
            m.mag_le += 1.0;
        },
        C::VIEW,
    ),
    (
        "mag_rcu",
        |m| {
            m.mag_rcu += 1.0;
        },
        C::VIEW,
    ),
    (
        "mag_plot",
        |m| {
            m.mag_plot = MagPlot::BH;
        },
        C::VIEW,
    ),
    (
        "mag_cur",
        |m| {
            m.mag_cur += 1;
        },
        C::NONE,
    ),
    (
        "inv_page",
        |m| {
            m.inv_page = InvPage::Mech;
        },
        C::VIEW,
    ),
    (
        "inv_a",
        |m| {
            m.inv_a = WaveId::Ref(1);
        },
        C::VIEW,
    ),
    (
        "inv_b",
        |m| {
            m.inv_b = WaveId::Ref(1);
        },
        C::VIEW,
    ),
    (
        "inv_c",
        |m| {
            m.inv_c = WaveId::Ref(1);
        },
        C::VIEW,
    ),
    (
        "inv_ang",
        |m| {
            m.inv_ang = InvAngle::Phase;
        },
        C::VIEW,
    ),
    (
        "inv_poles",
        |m| {
            m.inv_poles += 1;
        },
        C::VIEW,
    ),
    (
        "inv_an",
        |m| {
            m.inv_an = WaveId::Ref(1);
        },
        C::VIEW,
    ),
    (
        "inv_volt",
        |m| {
            m.inv_volt = !m.inv_volt;
        },
        C::VIEW,
    ),
    (
        "inv_tq",
        |m| {
            m.inv_tq = Some(WaveId::Ref(1));
        },
        C::VIEW,
    ),
    (
        "inv_ev",
        |m| {
            m.inv_ev = Some(WaveId::Ref(1));
        },
        C::VIEW,
    ),
    (
        "inv_ei",
        |m| {
            m.inv_ei = Some(WaveId::Ref(1));
        },
        C::VIEW,
    ),
    (
        "inv_scale",
        |m| {
            m.inv_scale += 1.0;
        },
        C::VIEW,
    ),
    (
        "inv_degv",
        |m| {
            m.inv_degv += 1.0;
        },
        C::VIEW,
    ),
    (
        "inv_off",
        |m| {
            m.inv_off += 1.0;
        },
        C::VIEW,
    ),
    (
        "inv_elec",
        |m| {
            m.inv_elec = !m.inv_elec;
        },
        C::VIEW,
    ),
    (
        "inv_qa",
        |m| {
            m.inv_qa += 1;
        },
        C::VIEW,
    ),
    (
        "inv_qb",
        |m| {
            m.inv_qb += 1;
        },
        C::VIEW,
    ),
    (
        "inv_qi",
        |m| {
            m.inv_qi = Some(1);
        },
        C::VIEW,
    ),
    (
        "inv_cpr",
        |m| {
            m.inv_cpr += 1.0;
        },
        C::VIEW,
    ),
    (
        "inv_swap",
        |m| {
            m.inv_swap = !m.inv_swap;
        },
        C::VIEW,
    ),
    (
        "inv_ha",
        |m| {
            m.inv_ha += 1;
        },
        C::VIEW,
    ),
    (
        "inv_hb",
        |m| {
            m.inv_hb += 1;
        },
        C::VIEW,
    ),
    (
        "inv_hc",
        |m| {
            m.inv_hc += 1;
        },
        C::VIEW,
    ),
    (
        "dpt_v",
        |m| {
            m.dpt_v = WaveId::Ref(1);
        },
        C::VIEW,
    ),
    (
        "dpt_i",
        |m| {
            m.dpt_i = WaveId::Ref(1);
        },
        C::VIEW,
    ),
    (
        "dpt_g",
        |m| {
            m.dpt_g = WaveId::Ref(1);
        },
        C::VIEW,
    ),
    (
        "dpt_4th",
        |m| {
            m.dpt_4th = DptFourth::Gate;
        },
        C::VIEW,
    ),
    (
        "dpt_4ch",
        |m| {
            m.dpt_4ch = WaveId::Ref(1);
        },
        C::VIEW,
    ),
    (
        "dpt_pct",
        |m| {
            m.dpt_pct += 1.0;
        },
        C::VIEW,
    ),
    (
        "dpt_zoom",
        |m| {
            m.dpt_zoom = DptZoom::TurnOn;
        },
        C::VIEW,
    ),
];
