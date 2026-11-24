// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area = Area { descriptors: ROWS, adjust, activate, ..Area::EMPTY };

const ROWS: &[Descriptor] = {
    use Item::*;
    use Kind::*;
    &[
        Descriptor::new(Open(MenuId::Horizontal), Action, "Horizontal", |_| String::new(), |_| Vec::new(), |_| false),
        Descriptor::new(
            HScale,
            Value,
            "Time/div",
            |u| si_short(u.m.time_per_div() as f64, "s"),
            |_| Vec::new(),
            |_| true,
        ),
        Descriptor::new(HFine, Choice, "Fine time base", |u| onoff(u.m.fine_t), toggles, |_| true),
        Descriptor::new(
            HExpandI,
            Choice,
            "Expand around",
            |u| format!("{:?}", u.m.h_expand),
            |_| ["Center", "Left", "Right", "Trigger"].into_iter().map(String::from).collect(),
            |_| true,
        ),
        Descriptor::new(HDelay, Value, "Delay", |u| si(u.m.delay as f64, "s"), |_| Vec::new(), |_| true),
        Descriptor::new(HZero, Action, "Delay to 0", |_| String::new(), |_| Vec::new(), |_| true),
        Descriptor::new(AutoDeskew, Action, "Auto deskew", |_| String::new(), |_| Vec::new(), |_| true),
        Descriptor::new(ClearDeskew, Action, "Clear deskew", |_| String::new(), |_| Vec::new(), |_| true),
        Descriptor::new(Zoom, Choice, "Zoom", |u| onoff(u.m.zoom), toggles, |_| true),
        Descriptor::new(
            ZScale,
            Value,
            "Zoom time/div",
            |u| si_short(tscales()[u.m.zscale] as f64, "s"),
            |_| Vec::new(),
            |u| u.m.zoom,
        ),
        Descriptor::new(ZDelay, Value, "Zoom position", |u| si(u.m.zdelay as f64, "s"), |_| Vec::new(), |u| u.m.zoom),
        Descriptor::new(AutoRoll, Choice, "Auto Roll", |u| onoff(u.m.auto_acquisition.roll()), toggles, |_| true),
    ]
};

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
        Item::AutoRoll => ui.m.auto_acquisition.set_roll(!ui.m.auto_acquisition.roll()),
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

#[cfg(test)]
mod tests;
