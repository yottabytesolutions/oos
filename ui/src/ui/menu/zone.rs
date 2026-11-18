// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area = Area { descriptors: ROWS, adjust, activate, ..Area::EMPTY };

const ROWS: &[Descriptor] = {
    use Item::*;
    use Kind::*;
    &[
        Descriptor::new(Open(MenuId::Zone), Action, "Zones", |_| String::new(), |_| Vec::new(), |_| false),
        Descriptor::new(
            ZoneSel,
            Choice,
            "Zone",
            |u| format!("Z{}", u.zone_sel + 1),
            |_| ["Z1", "Z2"].into_iter().map(String::from).collect(),
            |_| true,
        ),
        Descriptor::new(ZoneOn, Choice, "Enable", |u| onoff(u.m.zones[u.zone_sel].on), toggles, |_| true),
        Descriptor::new(
            ZoneSrc,
            Choice,
            "Source",
            |u| format!("CH{}", u.m.zones[u.zone_sel].src + 1),
            |_| ch_names(),
            |_| true,
        ),
        Descriptor::new(
            ZoneMode,
            Choice,
            "Condition",
            |u| if u.m.zones[u.zone_sel].inside { "Intersect" } else { "Not intersect" }.into(),
            |_| ["Intersect", "Not intersect"].into_iter().map(String::from).collect(),
            |_| true,
        ),
        Descriptor::new(ZoneBox, Action, "Draw box", |_| String::new(), |_| Vec::new(), |_| true),
        Descriptor::new(ZoneShape, Action, "Draw shape", |_| String::new(), |_| Vec::new(), |_| true),
        Descriptor::new(ZoneClear, Action, "Delete", |_| String::new(), |_| Vec::new(), |_| true),
    ]
};

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
