// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area = Area { kind, label, activate, menu_items, ..Area::EMPTY };

fn kind(it: Item) -> Option<Kind> {
    Some(match it {
        Item::PickOpt(_) => Kind::Action,
        _ => return None,
    })
}

fn label(it: Item) -> Option<&'static str> {
    Some(match it {
        Item::PickOpt(_) => "",
        _ => return None,
    })
}

fn activate(ui: &mut Ui, it: Item) -> bool {
    match it {
        Item::PickOpt(k) if ui.pick.is_some_and(|p| Ui::hex_item(p.it)) => {
            hex_key(ui, k);
        }
        Item::PickOpt(k) if ui.pick.is_some_and(|p| p.it == Item::AfgFreqEntry) => ui.freq_key(k),
        Item::PickOpt(k) if ui.pick.is_some_and(|p| p.it == Item::WlanNet) => {
            ui.close_picker();
            ui.wlan_pick(k);
        }
        Item::PickOpt(k)
            if ui
                .pick
                .is_some_and(|p| matches!(p.it, Item::NtpServer | Item::TimeZone | Item::RemName | Item::RemAllow)) =>
        {
            let it = ui.pick.map(|p| p.it).unwrap_or(Item::TimeZone);
            ui.close_picker();
            ui.time_pick(it, k);
        }
        Item::PickOpt(k) if ui.pick.is_some_and(|p| p.it == Item::WlanForgetNet) => {
            ui.close_picker();
            ui.wlan_forget_pick(k);
        }
        Item::PickOpt(k) if ui.pick.is_some_and(|p| p.it == Item::SshRemoveKey) => {
            ui.close_picker();
            ui.ssh_remove_pick(k);
        }
        Item::PickOpt(k) if ui.pick.is_some_and(|p| p.it == Item::RefImport) => {
            let p = ui.ref_files.get(k).cloned();
            ui.close_picker();
            if let Some(p) = p {
                ui.ref_load_path(&p);
            }
        }
        Item::PickOpt(k) => {
            if let Some(p) = ui.pick {
                let opts = ui.options(p.it);
                ui.close_picker();
                if let Some(o) = opts.get(k) {
                    ui.select_option(p.it, o);
                }
            }
        }
        _ => {}
    }
    true
}

fn menu_items(ui: &Ui, menu: MenuId) -> Vec<Item> {
    match menu {
        MenuId::Pick => {
            let n = ui.pick.map_or(0, |p| ui.options(p.it).len());
            (0..n).map(Item::PickOpt).collect()
        }
        _ => Vec::new(),
    }
}

fn hex_key(ui: &mut Ui, k: usize) {
    // keypad: digits edit the entry, OK sets the value and goes back
    match KEYPAD.get(k).copied() {
        Some("OK") => {
            let v = u64::from_str_radix(if ui.keypad.is_empty() { "0" } else { &ui.keypad }, 16)
                .unwrap_or(0)
                .min(BUS_VAL_MAX);
            if let Some(p) = ui.pick {
                ui.set_hex(p.it, v);
            }
            ui.close_picker();
        }
        Some("Clear") => ui.keypad.clear(),
        Some("Del") => {
            ui.keypad.pop();
        }
        Some(d) if ui.keypad.len() < 10 => {
            if ui.keypad == "0" {
                ui.keypad.clear();
            }
            ui.keypad.push_str(d);
        }
        _ => {}
    }
}
