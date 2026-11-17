// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area = Area {
    label: Ui::monitor_label,
    value: Ui::monitor_value,
    adjust: Ui::monitor_adjust,
    options: Ui::monitor_options,
    ..Area::EMPTY
};
