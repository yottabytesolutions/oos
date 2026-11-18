// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area = Area { descriptors: &ROWS, ordered_rows, adjust, activate, ..Area::EMPTY };

const PROBES: [f32; 3] = [1.0, 10.0, 100.0];

fn probe_label(probe: f32) -> String {
    format!("{probe}x")
}

const SHARED: [Descriptor; 2] = [
    Descriptor::choice(Item::ChFine, "Fine scale", |u| onoff(u.m.fine_v), toggles),
    Descriptor::choice(
        Item::ChExpand,
        "Expand around",
        |u| if u.m.v_expand_gnd { "Ground" } else { "Center" }.into(),
        |_| ["Center", "Ground"].into_iter().map(String::from).collect(),
    ),
];

const ROWS: [Descriptor; NCH * 13 + SHARED.len()] = {
    let channels: [[Descriptor; 13]; NCH] = [channel::<0>(), channel::<1>(), channel::<2>(), channel::<3>()];
    let mut rows = [SHARED[0]; NCH * 13 + SHARED.len()];
    rows[1] = SHARED[1];
    let mut c = 0;
    while c < NCH {
        let mut i = 0;
        while i < channels[c].len() {
            rows[SHARED.len() + c * channels[c].len() + i] = channels[c][i];
            i += 1;
        }
        c += 1;
    }
    rows
};

fn ordered_rows(_: &Ui, menu: MenuId) -> Vec<&'static Descriptor> {
    use Item::*;
    let items: &[Item] = match menu {
        MenuId::Channel(c) => &[
            ChOn(c),
            ChScale(c),
            ChOffset(c),
            ChCoupling(c),
            ChBw(c),
            ChProbe(c),
            ChInvert(c),
            ChFine,
            ChLabel(c),
            Open(MenuId::ChMore(c)),
        ],
        MenuId::ChMore(c) => &[ChUnit(c), ChBias(c), ChBiasZero(c), ChSkew(c), AutoDeskew, ChExpand],
        _ => &[],
    };
    items.iter().filter_map(|&it| super::descriptor(it)).collect()
}

const fn channel<const C: usize>() -> [Descriptor; 13] {
    use Item::*;
    [
        Descriptor::choice(ChOn(C), "Display", |u| onoff(u.m.ch[C].on), toggles),
        Descriptor::value(ChScale(C), "Scale", |u| si_short(u.m.ch[C].volts_per_div() as f64, u.m.unit(C))),
        Descriptor::value(ChOffset(C), "Offset", |u| si(u.m.ch[C].offset as f64, u.m.unit(C))),
        Descriptor::choice(
            ChCoupling(C),
            "Coupling",
            |u| u.m.ch[C].coupling.name().into(),
            |_| board().couplings().iter().map(|c| c.name().into()).collect(),
        ),
        Descriptor::choice(
            ChBw(C),
            "Bandwidth",
            |u| BWS[u.m.ch[C].bw].0.into(),
            |_| BWS.iter().map(|b| b.0.into()).collect(),
        ),
        Descriptor::choice(
            ChProbe(C),
            "Probe",
            |u| probe_label(u.m.ch[C].probe),
            |_| PROBES.into_iter().map(probe_label).collect(),
        ),
        Descriptor::choice(ChInvert(C), "Invert", |u| onoff(u.m.ch[C].invert), toggles),
        Descriptor::action(ChLabel(C), "Name"),
        Descriptor::action(Open(MenuId::ChMore(C)), "More"),
        Descriptor::choice(
            ChUnit(C),
            "Unit",
            |u| u.m.unit(C).into(),
            |_| AmpUnit::ALL.iter().map(|u| u.name().into()).collect(),
        ),
        Descriptor::value(ChBias(C), "Zero bias", |u| si(u.m.ch[C].bias as f64, u.m.unit(C))),
        Descriptor::action(ChBiasZero(C), "Zero this input"),
        Descriptor::value(ChSkew(C), "Deskew", |u| si(u.m.ch[C].skew as f64, "s")),
    ]
}

fn adjust(ui: &mut Ui, it: Item, d: i32) -> bool {
    let m = &mut ui.m;
    let step = |i: usize, n: usize, d: i32| -> usize { (i as i32 + d).clamp(0, n as i32 - 1) as usize };
    let cyc = |i: usize, n: usize, d: i32| -> usize { (i as i32 + d).rem_euclid(n as i32) as usize };
    match it {
        Item::ChOn(c) => {
            m.ch[c].on = !m.ch[c].on && c < n_ch();
            if m.ch[c].on && m.run == RunState::Run {
                m.floor_live_tdiv();
            }
        }
        Item::ChCoupling(c) => {
            let all = board().couplings();
            let i = all.iter().position(|&x| x == m.ch[c].coupling).unwrap_or(0);
            m.ch[c].coupling = all[cyc(i, all.len(), d)];
        }
        Item::ChBw(c) => m.ch[c].bw = cyc(m.ch[c].bw, BWS.len(), d),
        Item::ChProbe(c) => {
            let all = PROBES;
            let i = all.iter().position(|&x| x == m.ch[c].probe).unwrap_or(0);
            m.ch[c].probe = all[cyc(i, 3, d)];
        }
        Item::ChInvert(c) => m.ch[c].invert = !m.ch[c].invert,
        Item::ChScale(c) => {
            let old = m.ch[c].volts_per_div();
            if m.fine_v {
                // row by row through the calibration table: each scale has its own record
                let rows = board().vscale_steps_nv();
                let now = VSCALES[m.ch[c].vscale] as f64 * m.ch[c].vfine as f64 * 1e9 * 0.999999;
                let at = rows.iter().position(|&k| k as f64 >= now).unwrap_or(rows.len() - 1) as i32;
                let want = (rows[(at + d).clamp(0, rows.len() as i32 - 1) as usize] as f64 * 1e-9) as f32;
                let i = VSCALES.iter().rposition(|&s| s <= want * 1.000001).unwrap_or(0);
                let f = want / VSCALES[i];
                (m.ch[c].vscale, m.ch[c].vfine) = (i, if (f - 1.0).abs() < 1e-6 { 1.0 } else { f });
            } else {
                m.ch[c].vscale = step(m.ch[c].vscale, VSCALES.len(), d);
                m.ch[c].vfine = 1.0;
            }
            // around ground: the zero line keeps its place on the screen
            if m.v_expand_gnd {
                m.ch[c].offset *= m.ch[c].volts_per_div() / old;
            }
        }
        Item::ChUnit(c) => {
            let i = AmpUnit::ALL.iter().position(|&u| u == m.ch[c].unit).unwrap_or(0);
            m.ch[c].unit = AmpUnit::ALL[cyc(i, AmpUnit::ALL.len(), d)];
        }
        Item::ChBias(c) => m.ch[c].bias += 0.01 * m.ch[c].volts_per_div() * d as f32,
        Item::ChSkew(c) => m.ch[c].skew = (m.ch[c].skew + 600e-12 * d as f32).clamp(-SKEW_MAX, SKEW_MAX),
        Item::ChExpand => m.v_expand_gnd = !m.v_expand_gnd,
        Item::ChFine => m.fine_v = !m.fine_v,
        Item::ChOffset(c) => m.ch[c].offset += d as f32 * m.ch[c].volts_per_div() / 25.0,
        _ => {}
    }
    false
}

fn activate(ui: &mut Ui, it: Item) -> bool {
    match it {
        Item::ChLabel(c) => ui.wlan.osk = Some(wlan::Osk::for_label(c, &ui.m.ch_label[c])),
        Item::ChBiasZero(c) => ui.zero_input(c),
        _ => {}
    }
    true
}
