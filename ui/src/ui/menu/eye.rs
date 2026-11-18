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
    ..Area::EMPTY
};

fn menu_items(_ui: &Ui, menu: MenuId) -> Vec<Item> {
    match menu {
        MenuId::Eye => vec![
            Item::EyeAct,
            Item::EyeOn,
            Item::EyePlace,
            Item::EyeSrc,
            Item::EyeRate,
            Item::EyeBer,
            Item::EyeB,
            Item::EyeHp,
            Item::EyePnLo,
            Item::EyePnHi,
            Item::EyeNpj,
            Item::EyeRail,
            Item::EyeNotch,
            Item::EyeClear,
        ],
        _ => Vec::new(),
    }
}

fn sources(ui: &Ui, menu: MenuId) -> Vec<WaveId> {
    let m = &ui.m;
    match menu {
        MenuId::Eye => vec![m.eye_src],
        _ => Vec::new(),
    }
}
