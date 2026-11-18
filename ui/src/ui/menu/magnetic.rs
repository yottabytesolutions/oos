// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area = Area { descriptors: ROWS, sources, adjust: Ui::magnetic_adjust, ..Area::EMPTY };

const ROWS: &[Descriptor] = {
    use Item::*;
    use Kind::*;
    &[
        Descriptor::new(Open(MenuId::Magnetic), Action, "Magnetic", |_| String::new(), |_| Vec::new(), |_| false),
        Descriptor::new(MagOn, Choice, "View", |u| onoff(u.m.view == AView::Magnetic), toggles, |_| true),
        Descriptor::new(MagV, Choice, "Voltage", |u| u.wave_name(u.m.mag_v), Ui::wave_names, |_| true),
        Descriptor::new(MagI, Choice, "Current", |u| u.wave_name(u.m.mag_i), Ui::wave_names, |_| true),
        Descriptor::new(MagN, Value, "Turns", |u| format!("{:.0}", u.m.mag_n), |_| Vec::new(), |_| true),
        Descriptor::new(MagAe, Value, "Area", |u| geometry(u.m.mag_ae, "m²"), |_| Vec::new(), |_| true),
        Descriptor::new(MagLe, Value, "Length", |u| geometry(u.m.mag_le, "m"), |_| Vec::new(), |_| true),
        Descriptor::new(
            MagRcu,
            Value,
            "Rcu",
            |u| if u.m.mag_rcu > 0.0 { si(u.m.mag_rcu, "Ω") } else { "0 Ω".into() },
            |_| Vec::new(),
            |_| true,
        ),
        Descriptor::new(
            MagPlot,
            Choice,
            "Plot",
            |u| if u.m.mag_plot == crate::model::MagPlot::BH && has_geometry(u) { "B-H" } else { "I-ψ" }.into(),
            |u| if has_geometry(u) { vec!["I-ψ".into(), "B-H".into()] } else { vec!["I-ψ".into()] },
            |_| true,
        ),
        Descriptor::new(
            MagCur,
            Value,
            "Cursor",
            |u| u.m.mag_cur.to_string(),
            |_| Vec::new(),
            |u| u.azr.mag.as_ref().is_some_and(|m| m.loops.iter().any(|l| !l.is_empty())),
        ),
    ]
};

fn geometry(x: f64, unit: &str) -> String {
    if x > 0.0 {
        si(x, unit)
    } else {
        "off".into()
    }
}

fn has_geometry(ui: &Ui) -> bool {
    ui.m.mag_ae > 0.0 && ui.m.mag_le > 0.0
}

fn sources(ui: &Ui, menu: MenuId) -> Vec<WaveId> {
    let m = &ui.m;
    match menu {
        MenuId::Magnetic => vec![m.mag_v, m.mag_i],
        _ => Vec::new(),
    }
}
