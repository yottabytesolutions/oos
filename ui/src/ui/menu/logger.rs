// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area = Area {
    kind,
    label,
    value,
    adjust,
    options,
    menu_items: |_, _| vec![Item::LogOn, Item::LogIvl, Item::LogInfo],
    ..Area::EMPTY
};

fn kind(it: Item) -> Option<Kind> {
    Some(match it {
        Item::LogIvl => Kind::Value,
        _ => return None,
    })
}

fn label(it: Item) -> Option<&'static str> {
    Some(match it {
        Item::Open(MenuId::Logger) => "Data logger",
        Item::LogOn => "Logging",
        Item::LogIvl => "Interval",
        Item::LogInfo => "File",
        _ => return None,
    })
}

fn value(ui: &Ui, it: Item) -> Option<String> {
    let m = &ui.m;
    let onoff = |b: bool| if b { "On".to_string() } else { "Off".to_string() };
    Some(match it {
        Item::LogOn => onoff(ui.log.file.is_some()),
        Item::LogIvl => {
            si_short(super::super::logger::LOG_STEPS[m.log_ivl.min(super::super::logger::LOG_STEPS.len() - 1)], "s")
        }
        Item::LogInfo => ui.log_info(),
        _ => return None,
    })
}

fn adjust(ui: &mut Ui, it: Item, d: i32) -> bool {
    let m = &mut ui.m;
    let step = |i: usize, n: usize, d: i32| -> usize { (i as i32 + d).clamp(0, n as i32 - 1) as usize };
    match it {
        Item::LogOn => {
            if ui.log.file.is_some() {
                ui.log_stop("Log ended");
            } else {
                ui.log_start();
            }
        }
        Item::LogIvl => m.log_ivl = step(m.log_ivl, super::super::logger::LOG_STEPS.len(), d),
        _ => {}
    }
    false
}

fn options(_ui: &Ui, it: Item) -> Option<Vec<String>> {
    let v: &[&str] = match it {
        Item::LogOn => &["Off", "On"],
        _ => &[],
    };
    Some(v.iter().map(|s| s.to_string()).collect())
}
