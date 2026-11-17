// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area = Area { kind, label, value, adjust, activate, options, menu_items, ..Area::EMPTY };

fn kind(it: Item) -> Option<Kind> {
    Some(match it {
        Item::ChBias(_) | Item::ChSkew(_) => Kind::Value,
        Item::ChScale(_) | Item::ChOffset(_) => Kind::Value,
        Item::ChBiasZero(_) | Item::ChLabel(_) => Kind::Action,
        _ => return None,
    })
}

fn label(it: Item) -> Option<&'static str> {
    Some(match it {
        Item::Open(MenuId::ChMore(_)) => "More",
        Item::ChOn(_) => "Display",
        Item::ChCoupling(_) => "Coupling",
        Item::ChBw(_) => "Bandwidth",
        Item::ChProbe(_) => "Probe",
        Item::ChInvert(_) => "Invert",
        Item::ChScale(_) => "Scale",
        Item::ChOffset(_) => "Offset",
        Item::ChLabel(_) => "Name",
        Item::ChFine => "Fine scale",
        Item::ChUnit(_) => "Unit",
        Item::ChBias(_) => "Zero bias",
        Item::ChBiasZero(_) => "Zero this input",
        Item::ChSkew(_) => "Deskew",
        Item::ChExpand => "Expand around",
        _ => return None,
    })
}

fn value(ui: &Ui, it: Item) -> Option<String> {
    let m = &ui.m;
    let onoff = |b: bool| if b { "On".to_string() } else { "Off".to_string() };
    Some(match it {
        Item::ChOn(c) => onoff(m.ch[c].on),
        Item::ChCoupling(c) => m.ch[c].coupling.name().into(),
        Item::ChBw(c) => BWS[m.ch[c].bw].0.into(),
        Item::ChProbe(c) => format!("{}x", m.ch[c].probe),
        Item::ChInvert(c) => onoff(m.ch[c].invert),
        Item::ChScale(c) => si_short(m.ch[c].volts_per_div() as f64, m.unit(c)),
        Item::ChOffset(c) => si(m.ch[c].offset as f64, m.unit(c)),
        Item::ChUnit(c) => m.unit(c).into(),
        Item::ChBias(c) => si(m.ch[c].bias as f64, m.unit(c)),
        Item::ChSkew(c) => si(m.ch[c].skew as f64, "s"),
        Item::ChExpand => if m.v_expand_gnd { "Ground" } else { "Center" }.into(),
        Item::ChFine => onoff(m.fine_v),
        _ => return None,
    })
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
            let all = [1.0, 10.0, 100.0];
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

fn options(_ui: &Ui, it: Item) -> Option<Vec<String>> {
    let v: &[&str] = match it {
        Item::ChOn(_) | Item::ChInvert(_) => &["Off", "On"],
        Item::ChCoupling(_) => return Some(board().couplings().iter().map(|c| c.name().to_string()).collect()),
        Item::ChProbe(_) => &["1x", "10x", "100x"],
        Item::ChFine => &["Off", "On"],
        Item::ChUnit(_) => &["V", "A", "W", "U"],
        Item::ChExpand => &["Center", "Ground"],
        Item::ChBw(_) => return Some(BWS.iter().map(|b| b.0.to_string()).collect()),
        _ => &[],
    };
    Some(v.iter().map(|s| s.to_string()).collect())
}

fn menu_items(_ui: &Ui, menu: MenuId) -> Vec<Item> {
    match menu {
        MenuId::Channel(c) => vec![
            Item::ChOn(c),
            Item::ChScale(c),
            Item::ChOffset(c),
            Item::ChCoupling(c),
            Item::ChBw(c),
            Item::ChProbe(c),
            Item::ChInvert(c),
            Item::ChFine,
            Item::ChLabel(c),
            Item::Open(MenuId::ChMore(c)),
        ],
        MenuId::ChMore(c) => vec![
            Item::ChUnit(c),
            Item::ChBias(c),
            Item::ChBiasZero(c),
            Item::ChSkew(c),
            Item::AutoDeskew,
            Item::ChExpand,
        ],
        _ => Vec::new(),
    }
}
