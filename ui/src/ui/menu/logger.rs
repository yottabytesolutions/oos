// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area = Area { descriptors: ROWS, adjust, ..Area::EMPTY };

const ROWS: &[Descriptor] = {
    use Item::*;
    &[
        Descriptor::action(Open(MenuId::Logger), "Data logger").when(|_| false),
        Descriptor::choice(LogOn, "Logging", |u| onoff(u.log.file.is_some()), toggles),
        Descriptor::value(LogIvl, "Interval", |u| {
            si_short(super::super::logger::LOG_STEPS[u.m.log_ivl.min(super::super::logger::LOG_STEPS.len() - 1)], "s")
        }),
        Descriptor::choice(LogInfo, "File", Ui::log_info, |_| Vec::new()),
    ]
};

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
