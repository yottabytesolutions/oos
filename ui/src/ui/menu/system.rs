// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area = Area { kind, label, value, adjust, activate, options, menu_items, ..Area::EMPTY };

fn kind(it: Item) -> Option<Kind> {
    Some(match it {
        Item::About | Item::SysInfo | Item::Shot | Item::Stock | Item::Update | Item::UpdateRollback => Kind::Action,
        _ => return None,
    })
}

fn label(it: Item) -> Option<&'static str> {
    Some(match it {
        Item::Open(MenuId::Utility) => "System",
        Item::About => "About OOS",
        Item::SysInfo => "System info",
        Item::Shot => "Screenshot",
        Item::AuxOut => "AUX out",
        Item::Stock => "Back to stock",
        Item::Update => "Update",
        Item::BootMode => "Boot mode",
        Item::Overclock => "Overclock",
        Item::UpdateRollback => "Roll back update",
        Item::QuickAct => "Quick key",
        // drawn with the option text of the picker
        _ => return None,
    })
}

fn value(ui: &Ui, it: Item) -> Option<String> {
    let m = &ui.m;
    Some(match it {
        Item::AuxOut => m.aux_out.name().into(),
        Item::BootMode => ui.boot_mode().into(),
        Item::Overclock => overclock::label(ui.clock_next),
        Item::QuickAct => m.quick.name().into(),
        _ => return None,
    })
}

fn adjust(ui: &mut Ui, it: Item, d: i32) -> bool {
    if ui.aux_adjust(it, d) {
        return true;
    }
    let m = &mut ui.m;
    let cyc = |i: usize, n: usize, d: i32| -> usize { (i as i32 + d).rem_euclid(n as i32) as usize };
    match it {
        Item::QuickAct => {
            let i = QUICKS.iter().position(|&q| q == m.quick).unwrap_or(0);
            m.quick = QUICKS[cyc(i, QUICKS.len(), d)];
        }
        Item::BootMode => ui.boot_mode_toggle(),
        Item::Overclock => ui.overclock_step(d),
        _ => {}
    }
    false
}

fn activate(ui: &mut Ui, it: Item) -> bool {
    match it {
        Item::SysInfo => ui.sysinfo_open(),
        Item::About => {
            let p = oos_hal::model::platform();
            let build = ui.host.text("/etc/oos-build").unwrap_or_default();
            let mut d = vec![
                "OOS".into(),
                "An open operating system for the DHO800/DHO900".into(),
                if build.is_empty() {
                    format!("oos-ui {}", env!("CARGO_PKG_VERSION"))
                } else {
                    format!("Build {}", oos_hal::update::short(build.trim()))
                },
                ui.info.clone(),
                format!("Rigol {}", p.ident()),
                p.caps_line(),
            ];
            // device overrides (limits.conf) and detection problems: the details are on System info
            if p.experimental() {
                d.push(format!("Device overrides ({}), see System info", p.overrides.len()));
            }
            if !p.warnings.is_empty() {
                d.push(format!("{} warning(s), see System info", p.warnings.len()));
            }
            d.extend([
                format!("Source: {}   {:.0} fps", ui.src.name(), ui.fps),
                ui.start_up_summary().unwrap_or_default(),
                "Tap to close".into(),
            ]);
            d.retain(|l| !l.is_empty());
            ui.dialog = Some(d)
        }
        Item::Shot => {
            // without the dialog on it
            ui.close_menu();
            ui.shot_request = Some(String::new());
        }
        Item::Stock => ui.stock_tap(),
        Item::Update => ui.dialog = Some(ui.update_lines()),
        Item::UpdateRollback => ui.update_rollback_tap(),
        _ => {}
    }
    true
}

fn options(ui: &Ui, it: Item) -> Option<Vec<String>> {
    let v: &[&str] = match it {
        Item::AuxOut => {
            return Some(
                AuxOut::choices(board().mask_hw(ui.switches()) || board().zone_hw(ui.switches()))
                    .iter()
                    .map(|a| a.name().to_string())
                    .collect(),
            )
        }
        Item::BootMode => &["One-shot", "Persist"],
        Item::QuickAct => return Some(QUICKS.iter().map(|q| q.name().to_string()).collect()),
        Item::Overclock => return Some(board().clock_steps().iter().map(|&hz| overclock::label(hz)).collect()),
        _ => &[],
    };
    Some(v.iter().map(|s| s.to_string()).collect())
}

fn menu_items(_ui: &Ui, menu: MenuId) -> Vec<Item> {
    match menu {
        MenuId::Utility => vec![
            Item::Offline,
            Item::QuickAct,
            Item::AuxOut,
            Item::BootMode,
            Item::Overclock,
            Item::Graphics,
            Item::Shot,
            Item::Open(MenuId::SelfCal),
            Item::Open(MenuId::Network),
            Item::Open(MenuId::Features),
            Item::Open(MenuId::Time),
            Item::SysInfo,
            Item::About,
            Item::Update,
            Item::UpdateRollback,
            Item::Stock,
        ],
        _ => Vec::new(),
    }
}
