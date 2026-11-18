// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area = Area { descriptors: ROWS, adjust, ..Area::EMPTY };

const ROWS: &[Descriptor] = {
    use Item::*;
    use Kind::*;
    &[
        Descriptor::new(Open(MenuId::Cursor), Action, "Cursor", |_| String::new(), |_| Vec::new(), |_| false),
        Descriptor::new(
            CurMode,
            Choice,
            "Mode",
            |u| format!("{:?}", u.m.cursor),
            |_| ["Off", "Time", "Volt", "Track", "XY"].into_iter().map(String::from).collect(),
            |_| true,
        ),
        Descriptor::new(CurA, Value, "Cursor A", |u| position(u, u.m.cur_a, 0), |_| Vec::new(), enabled),
        Descriptor::new(
            CurB,
            Value,
            "Cursor B",
            |u| position(u, u.m.cur_b, if u.m.cursor == CursorMode::Track { 2 } else { 1 }),
            |_| Vec::new(),
            enabled,
        ),
        Descriptor::new(CurC, Value, "Y1", |u| position(u, u.m.cur_c, 2), |_| Vec::new(), xy),
        Descriptor::new(CurD, Value, "Y2", |u| position(u, u.m.cur_d, 3), |_| Vec::new(), xy),
        Descriptor::new(
            CurSrcB,
            Choice,
            "Cursor B source",
            |u| u.wave_name(u.cursor_b_source()),
            Ui::wave_names,
            |u| u.m.cursor == CursorMode::Track,
        ),
    ]
};

fn enabled(ui: &Ui) -> bool {
    ui.m.cursor != CursorMode::Off
}

fn xy(ui: &Ui) -> bool {
    ui.m.cursor == CursorMode::Xy
}

// Use the readout under the plot, or a percentage when it is unavailable.
fn position(ui: &Ui, fraction: f32, index: usize) -> String {
    ui.cursor_values().get(index).map_or(format!("{:.0} %", fraction * 100.0), |v| v.1.clone())
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

#[cfg(test)]
mod tests;
