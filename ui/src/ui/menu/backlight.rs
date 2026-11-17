// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area = Area {
    kind: Ui::backlight_kind,
    label: Ui::backlight_label,
    value: Ui::backlight_value,
    adjust: Ui::backlight_adjust,
    ..Area::EMPTY
};
