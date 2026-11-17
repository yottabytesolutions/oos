// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area = Area {
    kind: Ui::features_kind,
    label: Ui::features_label,
    value: Ui::features_value,
    adjust: Ui::features_adjust,
    options: Ui::features_options,
    menu_items: |ui, menu| ui.features_items(menu).unwrap_or_default(),
    ..Area::EMPTY
};
