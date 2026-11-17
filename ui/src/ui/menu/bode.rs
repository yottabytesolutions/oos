// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area = Area {
    sources,
    kind: Ui::bode_kind,
    label: Ui::bode_label,
    value: Ui::bode_value,
    adjust: Ui::bode_adjust,
    activate: Ui::bode_activate,
    options: Ui::bode_options,
    menu_items: |ui, _| ui.bode_items(),
};

fn sources(ui: &Ui, menu: MenuId) -> Vec<WaveId> {
    let m = &ui.m;
    let ch = WaveId::Ch;
    match menu {
        MenuId::Bode if m.bode_gen == super::super::bode::FREE => {
            [Some(m.bode_out), m.bode_sw_in.checked_sub(1).map(ch), Some(ch(m.bode_sync))]
                .into_iter()
                .flatten()
                .collect()
        }
        MenuId::Bode => vec![m.bode_in, m.bode_out],
        _ => Vec::new(),
    }
}
