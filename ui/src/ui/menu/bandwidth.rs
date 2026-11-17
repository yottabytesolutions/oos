// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area = Area {
    sources,
    kind: Ui::bw_kind,
    label: Ui::bw_label,
    value: Ui::bw_value,
    adjust: Ui::bw_adjust,
    options: Ui::bw_options,
    menu_items: |ui, _| ui.bw_items(),
    ..Area::EMPTY
};

fn sources(ui: &Ui, menu: MenuId) -> Vec<WaveId> {
    let m = &ui.m;
    let ch = WaveId::Ch;
    match menu {
        MenuId::BwTest => vec![ch(m.bw_src)],
        _ => Vec::new(),
    }
}
