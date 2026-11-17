// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area = Area { kind, label, activate, menu_items: |ui, _| ui.view_more_items(), ..Area::EMPTY };

fn kind(it: Item) -> Option<Kind> {
    Some(match it {
        Item::ViewExit | Item::ViewSettings => Kind::Action,
        _ => return None,
    })
}

fn label(it: Item) -> Option<&'static str> {
    Some(match it {
        Item::ViewExit => "Exit",
        Item::ViewSettings => "Settings",
        _ => return None,
    })
}

fn activate(ui: &mut Ui, it: Item) -> bool {
    match it {
        Item::ViewExit => ui.view_exit(),
        Item::ViewSettings => ui.view_settings(),
        _ => {}
    }
    true
}
