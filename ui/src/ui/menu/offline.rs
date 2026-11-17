// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area = Area {
    kind: Ui::offline_kind,
    label: Ui::offline_label,
    value: Ui::offline_value,
    adjust: Ui::offline_adjust,
    options: Ui::offline_options,
    ..Area::EMPTY
};
