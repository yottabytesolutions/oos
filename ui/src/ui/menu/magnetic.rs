// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area = Area {
    sources,
    kind: Ui::magnetic_kind,
    label,
    value: Ui::magnetic_value,
    adjust: Ui::magnetic_adjust,
    options: Ui::magnetic_options,
    menu_items: |ui, _| ui.magnetic_items(),
    ..Area::EMPTY
};

fn label(it: Item) -> Option<&'static str> {
    if let Some(v) = Ui::magnetic_label(it) {
        return Some(v);
    }
    Some(match it {
        Item::Open(MenuId::Magnetic) => "Magnetic",
        _ => return None,
    })
}

fn sources(ui: &Ui, menu: MenuId) -> Vec<WaveId> {
    let m = &ui.m;
    match menu {
        MenuId::Magnetic => vec![m.mag_v, m.mag_i],
        _ => Vec::new(),
    }
}
