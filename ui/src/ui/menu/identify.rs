// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area = Area {
    kind: Ui::ana_kind,
    label: Ui::ana_label,
    value: Ui::ana_value,
    adjust: Ui::ana_adjust,
    activate: Ui::ana_activate,
    options: Ui::ana_options,
    menu_items: |ui, _| ui.identify_items(),
    ..Area::EMPTY
};
