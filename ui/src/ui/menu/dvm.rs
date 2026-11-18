// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area = Area { descriptors: ROWS, sources, adjust, activate, ..Area::EMPTY };

const ROWS: &[Descriptor] = {
    use Item::*;
    use Kind::*;
    &[
        Descriptor::new(Open(MenuId::Dvm), Action, "DVM / counter", |_| String::new(), |_| Vec::new(), |_| false),
        Descriptor::new(DvmOn, Choice, "DVM", |u| onoff(u.m.dvm_on), toggles, |_| true),
        Descriptor::new(DvmSrc, Choice, "DVM source", |u| source(u, DvmSrc, u.m.dvm_src), sources_options, |_| true),
        Descriptor::new(
            DvmMode,
            Choice,
            "DVM mode",
            |u| tools::DVM_MODES[u.m.dvm_mode.min(2)].into(),
            |_| strings(&tools::DVM_MODES),
            |_| true,
        ),
        Descriptor::new(CntOn, Choice, "Counter", |u| onoff(u.m.cnt_on), toggles, |_| true),
        Descriptor::new(CntSrc, Choice, "Count source", |u| source(u, CntSrc, u.m.cnt_src), sources_options, |_| true),
        Descriptor::new(
            CntMode,
            Choice,
            "Count mode",
            |u| tools::CNT_MODES[u.m.cnt_mode.min(2)].into(),
            |_| strings(&tools::CNT_MODES),
            |_| true,
        ),
        Descriptor::new(CntDigits, Value, "Digits", |u| u.m.cnt_digits.to_string(), |_| Vec::new(), |_| true),
        Descriptor::new(CntClear, Action, "Clear counter", |_| String::new(), |_| Vec::new(), |_| true),
    ]
};

fn strings(labels: &[&str]) -> Vec<String> {
    labels.iter().map(|s| (*s).into()).collect()
}

fn source(ui: &Ui, it: Item, channel: usize) -> String {
    ui.offline_value(it).unwrap_or_else(|| format!("CH{}", channel + 1))
}

fn sources_options(ui: &Ui) -> Vec<String> {
    ui.offline_options(Item::DvmSrc).unwrap_or_else(ch_names)
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

#[cfg(test)]
mod tests;
