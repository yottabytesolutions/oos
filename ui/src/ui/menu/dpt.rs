// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area = Area { descriptors: ROWS, sources, adjust: Ui::dpt_adjust, ..Area::EMPTY };

const ROWS: &[Descriptor] = {
    use Item::*;
    use Kind::*;
    &[
        Descriptor::new(Open(MenuId::Dpt), Action, "Double pulse", |_| String::new(), |_| Vec::new(), |_| false),
        Descriptor::new(DptOn, Choice, "View", |u| onoff(u.m.view == AView::Dpt), toggles, |_| true),
        Descriptor::new(DptV, Choice, "Vds", |u| u.wave_name(u.m.dpt_v), Ui::wave_names, |_| true),
        Descriptor::new(DptI, Choice, "Id", |u| u.wave_name(u.m.dpt_i), Ui::wave_names, |_| true),
        Descriptor::new(DptG, Choice, "Vgs", |u| u.wave_name(u.m.dpt_g), Ui::wave_names, |_| true),
        Descriptor::new(
            DptFourth,
            Choice,
            "Fourth",
            |u| {
                match u.m.dpt_4th {
                    crate::model::DptFourth::Off => "Off",
                    crate::model::DptFourth::Gate => "Gate",
                    crate::model::DptFourth::DiodeV => "Diode V",
                    crate::model::DptFourth::DiodeI => "Diode I",
                }
                .into()
            },
            |_| ["Off", "Gate", "Diode V", "Diode I"].into_iter().map(String::from).collect(),
            |_| true,
        ),
        Descriptor::new(
            Dpt4,
            Choice,
            "Channel",
            |u| u.wave_name(u.m.dpt_4ch),
            Ui::wave_names,
            |u| u.m.dpt_4th != crate::model::DptFourth::Off,
        ),
        Descriptor::new(DptPct, Value, "Gate", |u| format!("{:.0} %", u.m.dpt_pct), |_| Vec::new(), |_| true),
        Descriptor::new(
            DptZoom,
            Choice,
            "Zoom",
            |u| {
                match u.m.dpt_zoom {
                    crate::model::DptZoom::Both => "Both",
                    crate::model::DptZoom::TurnOn => "Turn-on",
                    crate::model::DptZoom::TurnOff => "Turn-off",
                }
                .into()
            },
            |_| ["Both", "Turn-on", "Turn-off"].into_iter().map(String::from).collect(),
            |_| true,
        ),
    ]
};

fn sources(ui: &Ui, menu: MenuId) -> Vec<WaveId> {
    let m = &ui.m;
    match menu {
        MenuId::Dpt => {
            let mut v = vec![m.dpt_v, m.dpt_i, m.dpt_g];
            if m.dpt_4th != DptFourth::Off {
                v.push(m.dpt_4ch);
            }
            v
        }
        _ => Vec::new(),
    }
}
