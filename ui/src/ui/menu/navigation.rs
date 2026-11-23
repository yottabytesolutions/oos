// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area = Area {
    kind,
    label,
    activate,
    menu_items: |ui, _| ui.main_groups().into_iter().flat_map(|g| g.1).collect(),
    ..Area::EMPTY
};

fn kind(it: Item) -> Option<Kind> {
    Some(match it {
        Item::SrcOn => Kind::Action,
        Item::RunStop | Item::SingleAct | Item::AutosetAct | Item::DefaultAct | Item::SlopeAct | Item::ForceAct => {
            Kind::Action
        }
        Item::NavHist | Item::NavBack | Item::NavFwd | Item::QuickRun | Item::LockAct | Item::PowerAct => Kind::Action,
        _ => return None,
    })
}

fn label(it: Item) -> Option<&'static str> {
    Some(match it {
        Item::SrcOn => "Switch on source",
        Item::RunStop => "Run/Stop",
        Item::SingleAct => "Single",
        Item::AutosetAct => "Autoset",
        Item::DefaultAct => "Default",
        Item::SlopeAct => "Slope",
        Item::ForceAct => "Force",
        Item::NavHist => "History",
        Item::NavBack => "Older",
        Item::NavFwd => "Newer",
        // drawn with the action (see draw_menu)
        Item::QuickRun => "Quick",
        Item::LockAct => "Touch lock",
        Item::PowerAct => "Power off",
        _ => return None,
    })
}

fn activate(ui: &mut Ui, it: Item) -> bool {
    match it {
        Item::RunStop | Item::SingleAct | Item::AutosetAct | Item::DefaultAct | Item::ForceAct | Item::LockAct => {
            ui.close_menu();
            ui.key(match it {
                Item::RunStop => key::RUNSTOP,
                Item::SingleAct => key::SINGLE,
                Item::AutosetAct => key::AUTO,
                Item::DefaultAct => key::DEFAULT,
                Item::ForceAct => key::FORCE,
                _ => key::TOUCHLOCK,
            });
        }
        Item::SlopeAct => ui.key(key::SLOPE),
        Item::NavHist => ui.key(key::NAVIGATE),
        Item::NavBack => ui.key(0x08),
        Item::NavFwd => ui.key(0x06),
        Item::QuickRun => {
            ui.close_menu();
            ui.quick();
        }
        Item::PowerAct => {
            // the dialog of the POWER key, without the MCU answer that belongs to a key press
            crate::ui::power::open(ui);
        }
        Item::SrcOn => {
            let v = ui.off_sources(ui.menu);
            for &c in &v {
                ui.m.ch[c].on = true;
            }
            ui.focus = 0;
            change::edited(ui);
            let names: Vec<String> = v.iter().map(|c| format!("CH{}", c + 1)).collect();
            ui.toast(&format!("{} on", names.join(", ")));
        }
        _ => {}
    }
    true
}

pub(super) fn open(ui: &mut Ui, it: Item) {
    match it {
        Item::Open(MenuId::La) if !ui.m.la.on && !ui.la_probe_ok() => ui.toast("No LA probe connected"),
        // start menu > Measure: the measurement page, as the Measure button and key open it
        Item::Open(MenuId::Measure) if ui.menu == MenuId::Main => {
            ui.ensure_meas_own();
            ui.open_submenu(MenuId::MeasAdd);
        }
        Item::Open(id) => {
            if id == MenuId::Cursor && ui.m.cursor == CursorMode::Off {
                // like the CURSOR key
                ui.m.cursor = CursorMode::Time;
            }
            ui.open_submenu(id);
        }
        _ => {}
    }
}

pub(in crate::ui) fn main_groups(ui: &Ui) -> Vec<(&'static str, Vec<Item>)> {
    use Item::*;
    let mut disp = vec![Open(MenuId::Display), Open(MenuId::Horizontal), NavHist];
    if ui.hist_pos.is_some() {
        disp.extend([NavBack, NavFwd]);
    }
    // System > Features: what is left out is not offered
    let ana: Vec<Item> = super::super::analysers::ANALYSERS
        .iter()
        .flat_map(|a| a.start())
        .filter(|it| !matches!(it, Open(m) if ui.menu_left_out(*m)))
        .collect();
    // the generator page only on models with one (S models), the LA only with the port
    let mut ins = Vec::new();
    if oos_hal::model::platform().caps.has_gen() {
        ins.push(Open(MenuId::Afg));
    }
    if la::available() {
        ins.push(Open(MenuId::La));
    }
    ins.push(Open(MenuId::Dvm));
    ins.push(Open(MenuId::Logger));
    let mut g = vec![
        ("Acquisition", vec![RunStop, SingleAct, AutosetAct, ClearDisp, DefaultAct, Open(MenuId::Acquire)]),
        ("Trigger", vec![Open(MenuId::Trigger), SlopeAct, ForceAct, Open(MenuId::Zone)]),
        (
            "Analysis",
            [
                Open(MenuId::Measure),
                Open(MenuId::Cursor),
                Open(MenuId::Math),
                Open(MenuId::Ref),
                Open(MenuId::Sim),
                Open(MenuId::Decode),
                Open(MenuId::Bus),
                Open(MenuId::Mask),
            ]
            .into_iter()
            .filter(|it| !matches!(it, Open(m) if ui.menu_left_out(*m)))
            .collect(),
        ),
        ("Analysers", ana),
        ("Instruments", ins),
        ("Display", disp),
        ("System", vec![Open(MenuId::Utility), Open(MenuId::Storage), Shot, QuickRun, LockAct, PowerAct]),
    ];
    g.retain(|x| !x.1.is_empty());
    g
}
