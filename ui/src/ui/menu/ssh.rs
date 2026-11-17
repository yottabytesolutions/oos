// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area = Area {
    kind: Ui::remote_kind,
    label: Ui::remote_label,
    value: Ui::remote_value,
    adjust: Ui::remote_adjust,
    activate: Ui::ssh_activate,
    options: Ui::remote_options,
    menu_items: |ui, _| ui.ssh_items(),
    ..Area::EMPTY
};
