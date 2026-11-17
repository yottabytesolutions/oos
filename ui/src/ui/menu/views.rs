// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area = Area {
    kind: Ui::views_kind,
    label: Ui::views_label,
    value: Ui::views_value,
    adjust: Ui::views_adjust,
    options: Ui::views_options,
    ..Area::EMPTY
};
