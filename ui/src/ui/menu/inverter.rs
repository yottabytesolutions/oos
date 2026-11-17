// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area = Area {
    sources,
    kind: Ui::inverter_kind,
    label,
    value: Ui::inverter_value,
    adjust: Ui::inverter_adjust,
    options: Ui::inverter_options,
    menu_items: |ui, _| ui.inverter_items(),
    ..Area::EMPTY
};

fn label(it: Item) -> Option<&'static str> {
    if let Some(v) = Ui::inverter_label(it) {
        return Some(v);
    }
    Some(match it {
        Item::Open(MenuId::Inverter) => "Inverter",
        _ => return None,
    })
}

fn sources(ui: &Ui, menu: MenuId) -> Vec<WaveId> {
    let m = &ui.m;
    match menu {
        MenuId::Inverter if m.inv_page == InvPage::Dq0 => {
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
