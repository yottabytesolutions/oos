// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area = Area {
    kind: Ui::sim_kind,
    label: Ui::sim_label,
    value: Ui::sim_value,
    adjust: Ui::sim_adjust,
    activate: Ui::sim_activate,
    options: Ui::sim_options,
    menu_items,
    ..Area::EMPTY
};

fn menu_items(ui: &Ui, menu: MenuId) -> Vec<Item> {
    match menu {
        MenuId::Sim => ui.sim_items(),
        MenuId::SimAdvanced => ui.sim_advanced_items(),
        _ => Vec::new(),
    }
}
