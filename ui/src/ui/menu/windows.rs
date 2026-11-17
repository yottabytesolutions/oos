// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area = Area {
    kind: Ui::win_kind,
    label: Ui::win_label,
    value: Ui::win_value,
    adjust: Ui::win_adjust,
    options: Ui::win_options,
    menu_items: |ui, _| ui.win_items(),
    ..Area::EMPTY
};
