// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area = Area {
    kind: Ui::bus_kind,
    label: Ui::bus_label,
    value: Ui::bus_value,
    adjust: Ui::bus_adjust,
    activate,
    options: Ui::bus_options,
    menu_items: |ui, _| ui.bus_items(),
    ..Area::EMPTY
};

fn activate(ui: &mut Ui, it: Item) -> bool {
    if it == Item::BusLabel {
        ui.wlan.osk = Some(wlan::Osk::for_bus(&ui.m.bus.label));
    }
    true
}
