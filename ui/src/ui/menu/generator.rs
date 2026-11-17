// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area = Area {
    kind: Ui::afg_kind,
    label: Ui::afg_label,
    value: Ui::afg_value,
    adjust: Ui::afg_adjust,
    activate: Ui::afg_activate,
    options,
    menu_items: |ui, _| ui.afg_items(),
    ..Area::EMPTY
};

fn options(ui: &Ui, it: Item) -> Option<Vec<String>> {
    if let Some(v) = ui.afg_options(it) {
        return Some(v);
    }
    let v: &[&str] = match it {
        Item::AfgFreqEntry => return Some(FREQ_KEYS.iter().map(|k| k.to_string()).collect()),
        _ => &[],
    };
    Some(v.iter().map(|s| s.to_string()).collect())
}
