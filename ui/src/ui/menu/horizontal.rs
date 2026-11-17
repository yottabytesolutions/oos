// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area = Area { kind, label, value, adjust, activate, options, menu_items, ..Area::EMPTY };

fn kind(it: Item) -> Option<Kind> {
    Some(match it {
        Item::ZScale | Item::ZDelay => Kind::Value,
        Item::HScale | Item::HDelay => Kind::Value,
        Item::HZero => Kind::Action,
        Item::AutoDeskew | Item::ClearDeskew => Kind::Action,
        _ => return None,
    })
}

fn label(it: Item) -> Option<&'static str> {
    Some(match it {
        Item::Open(MenuId::Horizontal) => "Horizontal",
        Item::Zoom => "Zoom",
        Item::AutoRoll => "Auto Roll",
        Item::ZScale => "Zoom time/div",
        Item::ZDelay => "Zoom position",
        Item::HScale => "Time/div",
        Item::HDelay => "Delay",
        Item::HZero => "Delay to 0",
        Item::AutoDeskew => "Auto deskew",
        Item::ClearDeskew => "Clear deskew",
        Item::HFine => "Fine time base",
        Item::HExpandI => "Expand around",
        _ => return None,
    })
}

fn value(ui: &Ui, it: Item) -> Option<String> {
    let m = &ui.m;
    let onoff = |b: bool| if b { "On".to_string() } else { "Off".to_string() };
    Some(match it {
        Item::HFine => onoff(m.fine_t),
        Item::HExpandI => format!("{:?}", m.h_expand),
        Item::HScale => si_short(m.time_per_div() as f64, "s"),
        Item::HDelay => si(m.delay as f64, "s"),
        Item::Zoom => onoff(m.zoom),
        Item::AutoRoll => onoff(m.auto_roll),
        Item::ZScale => si_short(tscales()[m.zscale] as f64, "s"),
        Item::ZDelay => si(m.zdelay as f64, "s"),
        _ => return None,
    })
}

fn adjust(ui: &mut Ui, it: Item, d: i32) -> bool {
    let m = &mut ui.m;
    let step = |i: usize, n: usize, d: i32| -> usize { (i as i32 + d).clamp(0, n as i32 - 1) as usize };
    let cyc = |i: usize, n: usize, d: i32| -> usize { (i as i32 + d).rem_euclid(n as i32) as usize };
    match it {
        Item::HFine => m.fine_t = !m.fine_t,
        Item::HExpandI => {
            let all = [HExpand::Center, HExpand::Left, HExpand::Right, HExpand::Trigger];
            let i = all.iter().position(|&x| x == m.h_expand).unwrap_or(0);
            m.h_expand = all[cyc(i, all.len(), d)];
        }
        Item::HScale => {
            let old = m.time_per_div();
            if m.fine_t {
                (m.tscale, m.tfine) = fine_time(old, d);
            } else {
                m.tscale = step(m.tscale, tscales().len(), d);
                m.tfine = 1.0;
            }
            // the delay is the time in the middle of the screen: keep the expand point where it is
            let new = m.time_per_div();
            let half = HDIV / 2.0;
            match m.h_expand {
                HExpand::Center => {}
                HExpand::Left => m.delay -= half * (old - new),
                HExpand::Right => m.delay += half * (old - new),
                HExpand::Trigger => m.delay *= new / old,
            }
        }
        Item::HDelay => m.delay += d as f32 * m.time_per_div() / 25.0,
        Item::Zoom => {
            let on = !ui.m.zoom;
            ui.set_zoom(on);
        }
        Item::AutoRoll => ui.m.auto_roll = !ui.m.auto_roll,
        Item::ZScale => {
            ui.m.zscale = step(ui.m.zscale, tscales().len(), d);
        }
        Item::ZDelay => {
            ui.m.zdelay += d as f32 * tscales()[ui.m.zscale] / 25.0;
        }
        _ => {}
    }
    false
}

fn activate(ui: &mut Ui, it: Item) -> bool {
    match it {
        Item::HZero => {
            ui.m.delay = 0.0;
            change::edited(ui);
        }
        Item::AutoDeskew => ui.auto_deskew(),
        Item::ClearDeskew => ui.clear_deskew(),
        _ => {}
    }
    true
}

fn options(_ui: &Ui, it: Item) -> Option<Vec<String>> {
    let v: &[&str] = match it {
        Item::Zoom | Item::AutoRoll => &["Off", "On"],
        Item::HFine => &["Off", "On"],
        Item::HExpandI => &["Center", "Left", "Right", "Trigger"],
        _ => &[],
    };
    Some(v.iter().map(|s| s.to_string()).collect())
}

fn menu_items(ui: &Ui, menu: MenuId) -> Vec<Item> {
    match menu {
        MenuId::Horizontal => {
            if ui.m.zoom {
                vec![
                    Item::HScale,
                    Item::HFine,
                    Item::HExpandI,
                    Item::HDelay,
                    Item::HZero,
                    Item::AutoDeskew,
                    Item::ClearDeskew,
                    Item::Zoom,
                    Item::ZScale,
                    Item::ZDelay,
                    Item::AutoRoll,
                ]
            } else {
                vec![
                    Item::HScale,
                    Item::HFine,
                    Item::HExpandI,
                    Item::HDelay,
                    Item::HZero,
                    Item::AutoDeskew,
                    Item::ClearDeskew,
                    Item::Zoom,
                    Item::AutoRoll,
                ]
            }
        }
        _ => Vec::new(),
    }
}
