// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area = Area {
    kind: Ui::la_kind,
    label: Ui::la_label,
    value: Ui::la_value,
    adjust: Ui::la_adjust,
    options: Ui::la_options,
    menu_items,
    ..Area::EMPTY
};

fn menu_items(ui: &Ui, menu: MenuId) -> Vec<Item> {
    match menu {
        MenuId::La => ui.la_items(),
        MenuId::LaLines => ui.la_line_items(),
        MenuId::LaPat => (0..16).map(Item::LaPat).collect(),
        _ => Vec::new(),
    }
}
