// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area = Area { kind, label, value, adjust, activate, options, menu_items, ..Area::EMPTY };

fn kind(it: Item) -> Option<Kind> {
    Some(match it {
        Item::ZoneBox | Item::ZoneShape | Item::ZoneClear => Kind::Action,
        _ => return None,
    })
}

fn label(it: Item) -> Option<&'static str> {
    Some(match it {
        Item::Open(MenuId::Zone) => "Zones",
        Item::ZoneSel => "Zone",
        Item::ZoneOn => "Enable",
        Item::ZoneSrc => "Source",
        Item::ZoneMode => "Condition",
        Item::ZoneBox => "Draw box",
        Item::ZoneShape => "Draw shape",
        Item::ZoneClear => "Delete",
        _ => return None,
    })
}

fn value(ui: &Ui, it: Item) -> Option<String> {
    let m = &ui.m;
    let onoff = |b: bool| if b { "On".to_string() } else { "Off".to_string() };
    Some(match it {
        Item::ZoneSel => format!("Z{}", ui.zone_sel + 1),
        Item::ZoneOn => onoff(m.zones[ui.zone_sel].on),
        Item::ZoneSrc => format!("CH{}", m.zones[ui.zone_sel].src + 1),
        Item::ZoneMode => {
            if m.zones[ui.zone_sel].inside {
                "Intersect".into()
            } else {
                "Not intersect".into()
            }
        }
        _ => return None,
    })
}

fn adjust(ui: &mut Ui, it: Item, d: i32) -> bool {
    let m = &mut ui.m;
    let cyc = |i: usize, n: usize, d: i32| -> usize { (i as i32 + d).rem_euclid(n as i32) as usize };
    match it {
        Item::ZoneSel => ui.zone_sel = cyc(ui.zone_sel, 2, d),
        Item::ZoneOn => {
            let z = &mut m.zones[ui.zone_sel];
            z.on = !z.on;
        }
        Item::ZoneSrc => {
            let z = &mut m.zones[ui.zone_sel];
            z.src = src_step(z.src, NCH, d);
        }
        Item::ZoneMode => {
            let z = &mut m.zones[ui.zone_sel];
            z.inside = !z.inside;
        }
        _ => {}
    }
    false
}

fn activate(ui: &mut Ui, it: Item) -> bool {
    match it {
        Item::ZoneBox | Item::ZoneShape => {
            ui.zone_arm = Some((ui.zone_sel, it == Item::ZoneShape));
            ui.close_menu();
            ui.toast(if it == Item::ZoneShape { "Draw the zone with one finger" } else { "Drag a box for the zone" });
        }
        Item::ZoneClear => {
            ui.m.zones[ui.zone_sel] = crate::zone::Zone { src: ui.m.zones[ui.zone_sel].src, ..Default::default() };
        }
        _ => {}
    }
    true
}

fn options(_ui: &Ui, it: Item) -> Option<Vec<String>> {
    let v: &[&str] = match it {
        Item::ZoneOn => &["Off", "On"],
        Item::ZoneSel => &["Z1", "Z2"],
        Item::ZoneSrc => return Some(ch_names()),
        Item::ZoneMode => &["Intersect", "Not intersect"],
        _ => &[],
    };
    Some(v.iter().map(|s| s.to_string()).collect())
}

fn menu_items(_ui: &Ui, menu: MenuId) -> Vec<Item> {
    match menu {
        MenuId::Zone => vec![
            Item::ZoneSel,
            Item::ZoneOn,
            Item::ZoneSrc,
            Item::ZoneMode,
            Item::ZoneBox,
            Item::ZoneShape,
            Item::ZoneClear,
        ],
        _ => Vec::new(),
    }
}
