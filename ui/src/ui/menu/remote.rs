// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area = Area {
    kind: Ui::remote_kind,
    label: Ui::remote_label,
    value: Ui::remote_value,
    adjust: Ui::remote_adjust,
    options: Ui::remote_options,
    menu_items: |ui, _| ui.remote_items(),
    ..Area::EMPTY
};
