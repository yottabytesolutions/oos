// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area = Area { descriptors: ROWS, adjust: Ui::backlight_adjust, ..Area::EMPTY };

const ROWS: &[Descriptor] = {
    use Item::*;
    &[Descriptor::value(Backlight, "Backlight", |u| format!("{} %", u.sys.backlight))]
};
