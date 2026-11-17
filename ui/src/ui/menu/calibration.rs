// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area = Area {
    kind: Ui::cal_kind,
    label: Ui::cal_label,
    value: Ui::cal_value,
    adjust: Ui::cal_adjust,
    activate: Ui::cal_activate,
    options: Ui::cal_options,
    menu_items: |ui, _| ui.cal_items(),
    ..Area::EMPTY
};
