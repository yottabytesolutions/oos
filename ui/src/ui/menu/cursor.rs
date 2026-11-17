// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area = Area { kind, label, value, adjust, options, menu_items, ..Area::EMPTY };

fn kind(it: Item) -> Option<Kind> {
    Some(match it {
        Item::CurC | Item::CurD => Kind::Value,
        Item::CurA | Item::CurB => Kind::Value,
        _ => return None,
    })
}

fn label(it: Item) -> Option<&'static str> {
    Some(match it {
        Item::Open(MenuId::Cursor) => "Cursor",
        Item::CurMode => "Mode",
        Item::CurSrcB => "Cursor B source",
        Item::CurA => "Cursor A",
        Item::CurB => "Cursor B",
        Item::CurC => "Y1",
        Item::CurD => "Y2",
        _ => return None,
    })
}

fn value(ui: &Ui, it: Item) -> Option<String> {
    let m = &ui.m;
    Some(match it {
        Item::CurSrcB => ui.wave_name(ui.cursor_b_source()),
        Item::CurC => ui.cursor_values().get(2).map_or(format!("{:.0} %", m.cur_c * 100.0), |v| v.1.clone()),
        Item::CurD => ui.cursor_values().get(3).map_or(format!("{:.0} %", m.cur_d * 100.0), |v| v.1.clone()),
        Item::CurMode => format!("{:?}", m.cursor),
        // where the cursor is, as the read-out under the plot says it; the percentage only without a record
        Item::CurA => ui.cursor_values().first().map_or(format!("{:.0} %", m.cur_a * 100.0), |v| v.1.clone()),
        Item::CurB => ui
            .cursor_values()
            .get(if m.cursor == CursorMode::Track { 2 } else { 1 })
            .map_or(format!("{:.0} %", m.cur_b * 100.0), |v| v.1.clone()),
        _ => return None,
    })
}

fn adjust(ui: &mut Ui, it: Item, d: i32) -> bool {
    let m = &mut ui.m;
    let cyc = |i: usize, n: usize, d: i32| -> usize { (i as i32 + d).rem_euclid(n as i32) as usize };
    match it {
        Item::CurSrcB => {
            let cur = ui.cursor_b_source();
            let next = ui.step_wave(cur, d);
            ui.set_cursor_b_source(next);
        }
        Item::CurMode => {
            let all = [CursorMode::Off, CursorMode::Time, CursorMode::Volt, CursorMode::Track, CursorMode::Xy];
            let i = all.iter().position(|&x| x == m.cursor).expect("all cursor modes are listed");
            m.cursor = all[cyc(i, all.len(), d)];
        }
        Item::CurA => m.cur_a = (m.cur_a + d as f32 * 0.005).clamp(0.0, 1.0),
        Item::CurB => m.cur_b = (m.cur_b + d as f32 * 0.005).clamp(0.0, 1.0),
        Item::CurC => m.cur_c = (m.cur_c + d as f32 * 0.005).clamp(0.0, 1.0),
        Item::CurD => m.cur_d = (m.cur_d + d as f32 * 0.005).clamp(0.0, 1.0),
        _ => {}
    }
    false
}

fn options(ui: &Ui, it: Item) -> Option<Vec<String>> {
    let v: &[&str] = match it {
        Item::CurSrcB => return Some(ui.wave_names()),
        Item::CurMode => &["Off", "Time", "Volt", "Track", "XY"],
        _ => &[],
    };
    Some(v.iter().map(|s| s.to_string()).collect())
}

fn menu_items(ui: &Ui, menu: MenuId) -> Vec<Item> {
    match menu {
        MenuId::Cursor if ui.m.cursor == CursorMode::Off => vec![Item::CurMode],
        MenuId::Cursor if ui.m.cursor == CursorMode::Xy => {
            vec![Item::CurMode, Item::CurA, Item::CurB, Item::CurC, Item::CurD]
        }
        MenuId::Cursor if ui.m.cursor == CursorMode::Track => {
            vec![Item::CurMode, Item::CurA, Item::CurB, Item::CurSrcB]
        }
        MenuId::Cursor => vec![Item::CurMode, Item::CurA, Item::CurB],
        _ => Vec::new(),
    }
}
