// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::super::motor::short_num;
use super::*;
use crate::model::InvPage as Page;

pub(super) const AREA: Area = Area { descriptors: ROWS, rows, sources, adjust: Ui::inverter_adjust, ..Area::EMPTY };

const ROWS: &[Descriptor] = &[
    INV_ON, INV_PAGE, INV_A, INV_B, INV_C, INV_QTY, INV_ANG, INV_AN, INV_POLES, INV_DEG, INV_OFF, INV_ELEC, INV_QA,
    INV_QB, INV_QI, INV_CPR, INV_SWAP, INV_HA, INV_HB, INV_HC, INV_TQ, INV_SCALE, INV_EV, INV_EI, TITLE,
];

const INV_ON: Descriptor = Descriptor::choice(Item::InvOn, "View", |u| onoff(u.m.view == AView::Inverter), toggles);

const INV_PAGE: Descriptor = Descriptor::choice(
    Item::InvPage,
    "Page",
    |u| if u.m.inv_page == Page::Dq0 { "DQ0" } else { "Mechanical" }.into(),
    |_| ["DQ0", "Mechanical"].into_iter().map(String::from).collect(),
);

const INV_A: Descriptor = Descriptor::choice(Item::InvA, "Phase A", |u| u.wave_name(u.m.inv_a), |u| u.wave_names())
    .when(|u| u.m.inv_page == Page::Dq0 || matches!(u.m.inv_ang, InvAngle::Off | InvAngle::Phase));

const INV_B: Descriptor = Descriptor::choice(Item::InvB, "Phase B", |u| u.wave_name(u.m.inv_b), |u| u.wave_names())
    .when(|u| u.m.inv_page == Page::Dq0 || matches!(u.m.inv_ang, InvAngle::Off | InvAngle::Phase));

const INV_C: Descriptor = Descriptor::choice(Item::InvC, "Phase C", |u| u.wave_name(u.m.inv_c), |u| u.wave_names())
    .when(|u| u.m.inv_page == Page::Dq0 || matches!(u.m.inv_ang, InvAngle::Off | InvAngle::Phase));

const INV_QTY: Descriptor = Descriptor::choice(
    Item::InvQty,
    "Signal",
    |u| if u.m.inv_volt { "Voltage" } else { "Current" }.into(),
    |_| ["Voltage", "Current"].into_iter().map(String::from).collect(),
)
.when(|u| u.m.inv_page == Page::Dq0);

const INV_ANG: Descriptor = Descriptor::choice(Item::InvAng, "Angle", Ui::ang_name, |u| {
    let names = ["Off", "Phase", "Analog", "Quad", "Hall"];
    names[usize::from(u.m.inv_page == Page::Mech)..].iter().map(|s| (*s).into()).collect()
});

const INV_AN: Descriptor = Descriptor::choice(Item::InvAn, "Analog", |u| u.wave_name(u.m.inv_an), |u| u.wave_names())
    .when(|u| u.m.inv_ang == InvAngle::Analog);

const INV_POLES: Descriptor =
    Descriptor::value(Item::InvPoles, "Poles", |u| u.m.inv_poles.max(1).to_string()).when(|u| {
        matches!(u.m.inv_ang, InvAngle::Phase | InvAngle::Off) || (u.m.inv_ang == InvAngle::Analog && u.m.inv_elec)
    });

const INV_DEG: Descriptor =
    Descriptor::value(Item::InvDeg, "Deg/V", |u| format!("{} \u{b0}/V", short_num(u.m.inv_degv)))
        .when(|u| u.m.inv_ang == InvAngle::Analog);

const INV_OFF: Descriptor = Descriptor::value(Item::InvOff, "Offset", |u| format!("{} \u{b0}", short_num(u.m.inv_off)))
    .when(|u| u.m.inv_ang == InvAngle::Analog);

const INV_ELEC: Descriptor = Descriptor::choice(
    Item::InvElec,
    "Angle is",
    |u| if u.m.inv_elec { "Elec" } else { "Mech" }.into(),
    |_| ["Mech", "Elec"].into_iter().map(String::from).collect(),
)
.when(|u| u.m.inv_ang == InvAngle::Analog);

const INV_QA: Descriptor =
    Descriptor::value(Item::InvQa, "A lead", |u| line(u.m.inv_qa)).when(|u| u.m.inv_ang == InvAngle::Quad);

const INV_QB: Descriptor =
    Descriptor::value(Item::InvQb, "B", |u| line(u.m.inv_qb)).when(|u| u.m.inv_ang == InvAngle::Quad);

const INV_QI: Descriptor =
    Descriptor::value(Item::InvQi, "Index", |u| u.m.inv_qi.map(line).unwrap_or_else(|| "Off".into()))
        .when(|u| u.m.inv_ang == InvAngle::Quad);

const INV_CPR: Descriptor = Descriptor::value(Item::InvCpr, "Counts", |u| format!("{:.0}", u.m.inv_cpr))
    .when(|u| u.m.inv_ang == InvAngle::Quad);

const INV_SWAP: Descriptor =
    Descriptor::choice(Item::InvSwap, "Swap", |u| onoff(u.m.inv_swap), toggles).when(|u| u.m.inv_ang == InvAngle::Quad);

const INV_HA: Descriptor =
    Descriptor::value(Item::InvHa, "Hall A", |u| line(u.m.inv_ha)).when(|u| u.m.inv_ang == InvAngle::Hall);

const INV_HB: Descriptor =
    Descriptor::value(Item::InvHb, "Hall B", |u| line(u.m.inv_hb)).when(|u| u.m.inv_ang == InvAngle::Hall);

const INV_HC: Descriptor =
    Descriptor::value(Item::InvHc, "Hall C", |u| line(u.m.inv_hc)).when(|u| u.m.inv_ang == InvAngle::Hall);

const INV_TQ: Descriptor =
    Descriptor::choice(Item::InvTq, "Torque", |u| optional_wave(u, u.m.inv_tq), |u| u.wave_names_opt())
        .when(|u| u.m.inv_page == Page::Mech);

const INV_SCALE: Descriptor =
    Descriptor::value(Item::InvScale, "Scale", |u| short_num(u.m.inv_scale)).when(|u| u.m.inv_page == Page::Mech);

const INV_EV: Descriptor =
    Descriptor::choice(Item::InvEv, "Bus V", |u| optional_wave(u, u.m.inv_ev), |u| u.wave_names_opt())
        .when(|u| u.m.inv_page == Page::Mech);

const INV_EI: Descriptor =
    Descriptor::choice(Item::InvEi, "Bus I", |u| optional_wave(u, u.m.inv_ei), |u| u.wave_names_opt())
        .when(|u| u.m.inv_page == Page::Mech);

const TITLE: Descriptor = Descriptor::action(Item::Open(MenuId::Inverter), "Inverter").when(|_| false);

const MECHANICAL: &[Descriptor] = &[
    INV_ON, INV_PAGE, INV_ANG, INV_A, INV_B, INV_C, INV_AN, INV_DEG, INV_OFF, INV_ELEC, INV_POLES, INV_QA, INV_QB,
    INV_QI, INV_CPR, INV_SWAP, INV_HA, INV_HB, INV_HC, INV_TQ, INV_SCALE, INV_EV, INV_EI,
];

fn rows(ui: &Ui, _: MenuId) -> &'static [Descriptor] {
    if ui.m.inv_page == Page::Dq0 {
        ROWS
    } else {
        MECHANICAL
    }
}

fn line(n: u8) -> String {
    format!("D{}", n.min(15))
}

fn optional_wave(ui: &Ui, wave: Option<WaveId>) -> String {
    wave.map(|id| ui.wave_name(id)).unwrap_or_else(|| "Off".into())
}

fn sources(ui: &Ui, menu: MenuId) -> Vec<WaveId> {
    let m = &ui.m;
    match menu {
        MenuId::Inverter if m.inv_page == Page::Dq0 => {
            let mut v = vec![m.inv_a, m.inv_b, m.inv_c];
            if m.inv_ang == InvAngle::Analog {
                v.push(m.inv_an);
            }
            v
        }
        MenuId::Inverter => {
            let mut v = match m.inv_ang {
                InvAngle::Analog => vec![m.inv_an],
                InvAngle::Quad | InvAngle::Hall | InvAngle::Off => Vec::new(),
                InvAngle::Phase => vec![m.inv_a, m.inv_b, m.inv_c],
            };
            v.extend(m.inv_tq);
            v.extend(m.inv_ev);
            v.extend(m.inv_ei);
            v
        }
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests;
