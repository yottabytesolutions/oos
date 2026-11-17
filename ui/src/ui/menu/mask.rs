// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area = Area { sources, kind, label, value, adjust, activate, options, menu_items };

fn kind(it: Item) -> Option<Kind> {
    Some(match it {
        Item::MaskX | Item::MaskY => Kind::Value,
        Item::MaskCreate | Item::MaskFromRef | Item::MaskReset | Item::MaskClear => Kind::Action,
        Item::MaskSave | Item::MaskLoad => Kind::Action,
        _ => return None,
    })
}

fn label(it: Item) -> Option<&'static str> {
    Some(match it {
        Item::Open(MenuId::Mask) => "Mask",
        Item::MaskOn => "Mask test",
        Item::MaskSrc => "Source",
        Item::MaskX => "Tolerance X",
        Item::MaskY => "Tolerance Y",
        Item::MaskAction => "On fail",
        Item::MaskInvert => "Invert",
        Item::MaskCreate => "Create from CH",
        Item::MaskFromRef => "Create from ref",
        Item::MaskReset => "Reset counts",
        Item::MaskClear => "Clear mask",
        Item::MaskSave => "Save mask",
        Item::MaskLoad => "Load mask",
        _ => return None,
    })
}

fn value(ui: &Ui, it: Item) -> Option<String> {
    let m = &ui.m;
    let onoff = |b: bool| if b { "On".to_string() } else { "Off".to_string() };
    Some(match it {
        Item::MaskInvert => onoff(m.mask_inv),
        Item::MaskOn => onoff(m.mask_on),
        Item::MaskSrc => format!("CH{}", m.mask_src + 1),
        Item::MaskX => format!("{:.2} div", m.mask_x),
        Item::MaskY => format!("{:.2} div", m.mask_y),
        Item::MaskAction => {
            if m.mask_stop {
                "Stop".into()
            } else {
                "None".into()
            }
        }
        _ => return None,
    })
}

fn adjust(ui: &mut Ui, it: Item, d: i32) -> bool {
    match it {
        Item::MaskOn => {
            ui.m.mask_on = !ui.m.mask_on;
            if ui.m.mask_on && ui.mask.is_none() {
                ui.create_mask();
            }
        }
        Item::MaskSrc => {
            // a mask belongs to its channel: the new source needs a new one
            ui.m.mask_src = src_step(ui.m.mask_src, NCH, d);
            ui.mask = None;
            ui.mask_ref = None;
            ui.m.mask_on = false;
        }
        Item::MaskX => {
            ui.m.mask_x = ((ui.m.mask_x + d as f32 * 0.04) * 100.0).round().clamp(0.0, 200.0) / 100.0;
            ui.build_mask();
        }
        Item::MaskY => {
            ui.m.mask_y = ((ui.m.mask_y + d as f32 * 0.04) * 100.0).round().clamp(4.0, 400.0) / 100.0;
            ui.build_mask();
        }
        Item::MaskAction => ui.m.mask_stop = !ui.m.mask_stop,
        Item::MaskInvert => {
            ui.m.mask_inv = !ui.m.mask_inv;
            ui.build_mask();
        }
        _ => {}
    }
    false
}

fn activate(ui: &mut Ui, it: Item) -> bool {
    match it {
        Item::MaskSave => ui.mask_save(),
        Item::MaskLoad => ui.mask_load(),
        Item::MaskCreate => ui.create_mask(),
        Item::MaskFromRef => ui.create_mask_from_ref(),
        Item::MaskClear => {
            ui.mask = None;
            ui.mask_ref = None;
            ui.m.mask_on = false;
            change::mask_reset(ui);
            ui.toast("Mask cleared");
        }
        Item::MaskReset => change::mask_reset(ui),
        _ => {}
    }
    true
}

fn options(_ui: &Ui, it: Item) -> Option<Vec<String>> {
    let v: &[&str] = match it {
        Item::MaskOn | Item::MaskInvert => &["Off", "On"],
        Item::MaskSrc => return Some(ch_names()),
        Item::MaskAction => &["None", "Stop"],
        _ => &[],
    };
    Some(v.iter().map(|s| s.to_string()).collect())
}

fn menu_items(_ui: &Ui, menu: MenuId) -> Vec<Item> {
    match menu {
        MenuId::Mask => vec![
            Item::MaskOn,
            Item::MaskSrc,
            Item::MaskInvert,
            Item::MaskX,
            Item::MaskY,
            Item::MaskAction,
            Item::MaskCreate,
            Item::MaskFromRef,
            Item::MaskReset,
            Item::MaskClear,
            Item::MaskSave,
            Item::MaskLoad,
        ],
        _ => Vec::new(),
    }
}

fn sources(ui: &Ui, menu: MenuId) -> Vec<WaveId> {
    let m = &ui.m;
    let ch = WaveId::Ch;
    match menu {
        MenuId::Mask => vec![ch(m.mask_src)],
        _ => Vec::new(),
    }
}
