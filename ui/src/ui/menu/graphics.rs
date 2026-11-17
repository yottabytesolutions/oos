// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area = Area {
    kind: Ui::graphics_kind,
    label: Ui::graphics_label,
    value: Ui::graphics_value,
    adjust: Ui::graphics_adjust,
    options: Ui::graphics_options,
    ..Area::EMPTY
};
