// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area = Area { descriptors: ROWS, sources, adjust: Ui::bw_adjust, ..Area::EMPTY };

const ROWS: &[Descriptor] = {
    use Item::*;
    use Kind::*;
    &[
        Descriptor::new(Open(MenuId::BwTest), Action, "Bandwidth", |_| String::new(), |_| Vec::new(), |_| false),
        Descriptor::new(BwOn, Choice, "View", |u| onoff(u.m.view == AView::Step), toggles, |_| true),
        Descriptor::new(BwSrc, Choice, "Source", |u| format!("CH{}", u.m.bw_src + 1), |_| ch_names(), |_| true),
        Descriptor::new(
            BwHint,
            Choice,
            "Signal freq",
            |u| hint_name(u.m.bw_hint),
            |_| bwtest::HINTS.iter().map(|&h| hint_name(h)).collect(),
            |_| true,
        ),
        Descriptor::new(
            BwBins,
            Choice,
            "Bins / period",
            |u| u.m.bw_bins.to_string(),
            |_| bwtest::BINS.iter().map(ToString::to_string).collect(),
            |_| true,
        ),
    ]
};

fn hint_name(h: f64) -> String {
    if h == 0.0 {
        "Auto".into()
    } else {
        si_short(h, "Hz")
    }
}

fn sources(ui: &Ui, menu: MenuId) -> Vec<WaveId> {
    let m = &ui.m;
    let ch = WaveId::Ch;
    match menu {
        MenuId::BwTest => vec![ch(m.bw_src)],
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests;
