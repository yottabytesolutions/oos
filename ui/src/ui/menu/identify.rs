// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area =
    Area { descriptors: ROWS, adjust: Ui::ana_adjust, activate: Ui::ana_activate, ..Area::EMPTY };

const ROWS: &[Descriptor] = {
    use Item::*;
    &[
        Descriptor::action(IdentifyAct, "Identify").when(|_| false),
        Descriptor::choice(ProtoWatch, "Protocol watch", |u| onoff(u.azr.watch), toggles),
        // Each finding has its summary as its only option.
        Descriptor::choice(IdFind(0), "1", |u| finding(u, 0), |u| vec![finding(u, 0)])
            .when(|u| !u.azr.findings.is_empty()),
        Descriptor::choice(IdFind(1), "2", |u| finding(u, 1), |u| vec![finding(u, 1)])
            .when(|u| u.azr.findings.len() > 1),
        Descriptor::choice(IdFind(2), "3", |u| finding(u, 2), |u| vec![finding(u, 2)])
            .when(|u| u.azr.findings.len() > 2),
        Descriptor::choice(IdFind(3), "4", |u| finding(u, 3), |u| vec![finding(u, 3)])
            .when(|u| u.azr.findings.len() > 3),
        Descriptor::choice(IdFind(4), "5", |u| finding(u, 4), |u| vec![finding(u, 4)])
            .when(|u| u.azr.findings.len() > 4),
        Descriptor::choice(IdFind(5), "6", |u| finding(u, 5), |u| vec![finding(u, 5)])
            .when(|u| u.azr.findings.len() > 5),
        Descriptor::choice(IdFind(6), "7", |u| finding(u, 6), |u| vec![finding(u, 6)])
            .when(|u| u.azr.findings.len() > 6),
        Descriptor::choice(IdFind(7), "8", |u| finding(u, 7), |u| vec![finding(u, 7)])
            .when(|u| u.azr.findings.len() > 7),
    ]
};

// The renderer fits the full finding text into the row button.
fn finding(ui: &Ui, index: usize) -> String {
    ui.azr
        .findings
        .get(index)
        .map_or(String::new(), |f| format!("CH{} {}", f.channels.first().map_or(0, |c| c + 1), f.summary))
}
