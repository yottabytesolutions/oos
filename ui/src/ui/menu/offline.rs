// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area = Area { descriptors: ROWS, adjust: Ui::offline_adjust, ..Area::EMPTY };

const ROWS: &[Descriptor] = {
    use Item::*;
    &[Descriptor::choice(
        Offline,
        "Record",
        |u| {
            match u.look {
                reflook::RefLook::Live => "With live",
                reflook::RefLook::TakeOver => "Take over",
                reflook::RefLook::Waves => "Offline",
            }
            .into()
        },
        |_| ["With live", "Take over", "Offline"].into_iter().map(String::from).collect(),
    )]
};
