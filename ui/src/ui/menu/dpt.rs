// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area = Area {
    sources,
    kind: Ui::dpt_kind,
    label,
    value: Ui::dpt_value,
    adjust: Ui::dpt_adjust,
    options: Ui::dpt_options,
    menu_items: |ui, _| ui.dpt_items(),
    ..Area::EMPTY
};

fn label(it: Item) -> Option<&'static str> {
    if let Some(v) = Ui::dpt_label(it) {
        return Some(v);
    }
    Some(match it {
        Item::Open(MenuId::Dpt) => "Double pulse",
        _ => return None,
    })
}

fn sources(ui: &Ui, menu: MenuId) -> Vec<WaveId> {
    let m = &ui.m;
    match menu {
        MenuId::Dpt => {
            let mut v = vec![m.dpt_v, m.dpt_i, m.dpt_g];
            if m.dpt_4th != DptFourth::Off {
                v.push(m.dpt_4ch);
            }
            v
        }
        _ => Vec::new(),
    }
}
