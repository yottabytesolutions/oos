// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area = Area { descriptors: ROWS, sources, adjust, activate, ..Area::EMPTY };

const ROWS: &[Descriptor] = {
    use Item::*;
    use Kind::*;
    &[
        Descriptor::new(Open(MenuId::Mask), Action, "Mask", |_| String::new(), |_| Vec::new(), |_| false),
        Descriptor::new(MaskOn, Choice, "Mask test", |u| onoff(u.m.mask_on), toggles, |_| true),
        Descriptor::new(MaskSrc, Choice, "Source", |u| format!("CH{}", u.m.mask_src + 1), |_| ch_names(), |_| true),
        Descriptor::new(MaskInvert, Choice, "Invert", |u| onoff(u.m.mask_inv), toggles, |_| true),
        Descriptor::new(MaskX, Value, "Tolerance X", |u| format!("{:.2} div", u.m.mask_x), |_| Vec::new(), |_| true),
        Descriptor::new(MaskY, Value, "Tolerance Y", |u| format!("{:.2} div", u.m.mask_y), |_| Vec::new(), |_| true),
        Descriptor::new(
            MaskAction,
            Choice,
            "On fail",
            |u| if u.m.mask_stop { "Stop" } else { "None" }.into(),
            |_| ["None", "Stop"].into_iter().map(String::from).collect(),
            |_| true,
        ),
        Descriptor::new(MaskCreate, Action, "Create from CH", |_| String::new(), |_| Vec::new(), |_| true),
        Descriptor::new(MaskFromRef, Action, "Create from ref", |_| String::new(), |_| Vec::new(), |_| true),
        Descriptor::new(MaskReset, Action, "Reset counts", |_| String::new(), |_| Vec::new(), |_| true),
        Descriptor::new(MaskClear, Action, "Clear mask", |_| String::new(), |_| Vec::new(), |_| true),
        Descriptor::new(MaskSave, Action, "Save mask", |_| String::new(), |_| Vec::new(), |_| true),
        Descriptor::new(MaskLoad, Action, "Load mask", |_| String::new(), |_| Vec::new(), |_| true),
    ]
};

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

fn sources(ui: &Ui, menu: MenuId) -> Vec<WaveId> {
    let m = &ui.m;
    let ch = WaveId::Ch;
    match menu {
        MenuId::Mask => vec![ch(m.mask_src)],
        _ => Vec::new(),
    }
}
