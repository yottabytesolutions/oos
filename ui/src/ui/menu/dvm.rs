// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area = Area { sources, kind, label, value, adjust, activate, options, menu_items };

fn kind(it: Item) -> Option<Kind> {
    Some(match it {
        Item::CntDigits => Kind::Value,
        Item::CntClear => Kind::Action,
        _ => return None,
    })
}

fn label(it: Item) -> Option<&'static str> {
    Some(match it {
        Item::Open(MenuId::Dvm) => "DVM / counter",
        Item::DvmOn => "DVM",
        Item::DvmSrc => "DVM source",
        Item::DvmMode => "DVM mode",
        Item::CntOn => "Counter",
        Item::CntSrc => "Count source",
        Item::CntMode => "Count mode",
        Item::CntDigits => "Digits",
        Item::CntClear => "Clear counter",
        _ => return None,
    })
}

fn value(ui: &Ui, it: Item) -> Option<String> {
    if let Some(v) = ui.offline_value(it) {
        return Some(v);
    }
    let m = &ui.m;
    let onoff = |b: bool| if b { "On".to_string() } else { "Off".to_string() };
    Some(match it {
        Item::DvmOn => onoff(m.dvm_on),
        Item::DvmSrc => format!("CH{}", m.dvm_src + 1),
        Item::DvmMode => tools::DVM_MODES[m.dvm_mode.min(2)].into(),
        Item::CntOn => onoff(m.cnt_on),
        Item::CntSrc => format!("CH{}", m.cnt_src + 1),
        Item::CntMode => tools::CNT_MODES[m.cnt_mode.min(2)].into(),
        Item::CntDigits => m.cnt_digits.to_string(),
        _ => return None,
    })
}

fn adjust(ui: &mut Ui, it: Item, d: i32) -> bool {
    if ui.offline_adjust(it, d) {
        return true;
    }
    let m = &mut ui.m;
    match it {
        Item::DvmOn => m.dvm_on = !m.dvm_on,
        Item::DvmSrc => m.dvm_src = src_step(m.dvm_src, NCH, d),
        Item::DvmMode => m.dvm_mode = (m.dvm_mode as i32 + d).rem_euclid(3) as usize,
        Item::CntOn => m.cnt_on = !m.cnt_on,
        Item::CntSrc => m.cnt_src = src_step(m.cnt_src, NCH, d),
        Item::CntMode => m.cnt_mode = (m.cnt_mode as i32 + d).rem_euclid(3) as usize,
        Item::CntDigits => m.cnt_digits = (m.cnt_digits as i32 + d).clamp(3, 7) as u32,
        _ => {}
    }
    false
}

fn activate(ui: &mut Ui, it: Item) -> bool {
    if it == Item::CntClear {
        ui.counter_clear();
    }
    true
}

fn options(ui: &Ui, it: Item) -> Option<Vec<String>> {
    if let Some(v) = ui.offline_options(it) {
        return Some(v);
    }
    let v: &[&str] = match it {
        Item::DvmSrc | Item::CntSrc => return Some((0..n_ch()).map(|c| format!("CH{}", c + 1)).collect()),
        Item::DvmOn | Item::CntOn => &["Off", "On"],
        Item::DvmMode => &tools::DVM_MODES,
        Item::CntMode => &tools::CNT_MODES,
        _ => &[],
    };
    Some(v.iter().map(|s| s.to_string()).collect())
}

fn menu_items(_ui: &Ui, menu: MenuId) -> Vec<Item> {
    match menu {
        MenuId::Dvm => vec![
            Item::DvmOn,
            Item::DvmSrc,
            Item::DvmMode,
            Item::CntOn,
            Item::CntSrc,
            Item::CntMode,
            Item::CntDigits,
            Item::CntClear,
        ],
        // in the order of the groups of `measure_layout`
        _ => Vec::new(),
    }
}

fn sources(ui: &Ui, menu: MenuId) -> Vec<WaveId> {
    let m = &ui.m;
    let ch = WaveId::Ch;
    match menu {
        MenuId::Dvm if ui.offline => Vec::new(),
        MenuId::Dvm => {
            [m.dvm_on.then_some(ch(m.dvm_src)), m.cnt_on.then_some(ch(m.cnt_src))].into_iter().flatten().collect()
        }
        _ => Vec::new(),
    }
}
