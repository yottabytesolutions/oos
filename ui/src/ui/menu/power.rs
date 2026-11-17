// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area = Area {
    sources,
    kind: Ui::ana_kind,
    label: Ui::ana_label,
    value: Ui::ana_value,
    adjust: Ui::ana_adjust,
    activate: Ui::ana_activate,
    options: Ui::ana_options,
    menu_items,
};

fn menu_items(ui: &Ui, menu: MenuId) -> Vec<Item> {
    match menu {
        MenuId::Power => {
            let mut v = vec![Item::PowOn, Item::PowV, Item::PowI, Item::PowMode];
            match ui.m.pw_mode {
                PowerMode::Ripple => v.extend([Item::PowFline, Item::PowFsw, Item::PowVin]),
                PowerMode::Rail => v.extend([Item::PowRef, Item::PowVin, Item::PowClk]),
                _ => {}
            }
            v
        }
        _ => Vec::new(),
    }
}

fn sources(ui: &Ui, menu: MenuId) -> Vec<WaveId> {
    let m = &ui.m;
    match menu {
        MenuId::Power if matches!(m.pw_mode, PowerMode::Ripple | PowerMode::Rail) => vec![m.pw_v],
        MenuId::Power => vec![m.pw_v, m.pw_i],
        _ => Vec::new(),
    }
}
