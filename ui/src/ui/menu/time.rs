// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area = Area {
    kind: Ui::time_kind,
    label: Ui::time_label,
    value: Ui::time_value,
    adjust: Ui::time_adjust,
    activate: Ui::time_activate,
    options: Ui::time_options,
    menu_items: |ui, _| ui.time_items(),
    ..Area::EMPTY
};
