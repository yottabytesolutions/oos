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
        MenuId::Spectrum => vec![
            Item::SpecOn,
            Item::SpecSrc,
            Item::SpecCenter,
            Item::SpecSpan,
            Item::SpecHarm,
            Item::SpecRbw,
            Item::SpecWin,
            Item::SpecTrace,
            Item::SpecRef,
            Item::SpecWf,
            Item::SpecDist,
            Item::SpecAcprW,
            Item::SpecAcprS,
            Item::SpecPeak,
            Item::SpecMargin,
            Item::SpecLimitSet,
            Item::SpecLimitClear,
        ],
        _ => Vec::new(),
    }
}

fn sources(ui: &Ui, menu: MenuId) -> Vec<WaveId> {
    let m = &ui.m;
    match menu {
        MenuId::Spectrum => vec![m.spec_src],
        _ => Vec::new(),
    }
}
