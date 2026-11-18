// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area = Area { descriptors: ROWS, adjust, activate, ..Area::EMPTY };

const ROWS: &[Descriptor] = {
    use Item::*;
    &[
        Descriptor::title(Open(MenuId::Utility), "System"),
        Descriptor::reference(Offline),
        Descriptor::choice(
            QuickAct,
            "Quick key",
            |u| u.m.quick.name().into(),
            |_| QUICKS.iter().map(|q| q.name().into()).collect(),
        ),
        Descriptor::choice(
            AuxOut,
            "AUX out",
            |u| u.m.aux_out.name().into(),
            |u| {
                crate::model::AuxOut::choices(board().mask_hw(u.switches()) || board().zone_hw(u.switches()))
                    .iter()
                    .map(|a| a.name().into())
                    .collect()
            },
        ),
        Descriptor::choice(
            BootMode,
            "Boot mode",
            |u| u.boot_mode().into(),
            |_| ["One-shot", "Persist"].into_iter().map(String::from).collect(),
        ),
        Descriptor::choice(
            Overclock,
            "Overclock",
            |u| overclock::label(u.clock_next),
            |_| board().clock_steps().iter().map(|&hz| overclock::label(hz)).collect(),
        ),
        Descriptor::reference(Graphics),
        Descriptor::action(Shot, "Screenshot"),
        Descriptor::reference(Open(MenuId::SelfCal)),
        Descriptor::reference(Open(MenuId::Network)),
        Descriptor::reference(Open(MenuId::Features)),
        Descriptor::reference(Open(MenuId::Time)),
        Descriptor::action(SysInfo, "System info"),
        Descriptor::action(About, "About OOS"),
        Descriptor::action(Update, "Update"),
        Descriptor::action(UpdateRollback, "Roll back update"),
        Descriptor::action(Stock, "Back to stock"),
    ]
};

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

#[cfg(test)]
mod tests;
