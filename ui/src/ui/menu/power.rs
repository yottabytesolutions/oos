// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::super::analysis::POWER_MODES;
use super::*;

pub(super) const AREA: Area =
    Area { descriptors: ROWS, sources, adjust: Ui::ana_adjust, activate: Ui::ana_activate, ..Area::EMPTY };

const ROWS: &[Descriptor] = {
    use Item::*;
    use Kind::*;
    &[
        Descriptor::new(Open(MenuId::Power), Action, "Power analysis", |_| String::new(), |_| Vec::new(), |_| false),
        Descriptor::new(PowOn, Choice, "View", |u| onoff(u.m.view == AView::Power), toggles, |_| true),
        Descriptor::new(PowV, Choice, "Voltage", |u| u.wave_name(u.m.pw_v), Ui::wave_names, |_| true),
        Descriptor::new(PowI, Choice, "Current", |u| u.wave_name(u.m.pw_i), Ui::wave_names, |_| true),
        Descriptor::new(
            PowMode,
            Choice,
            "Page",
            |u| power_page(u.m.pw_mode).into(),
            |_| POWER_MODES.iter().map(|p| power_page(*p).to_string()).collect(),
            |_| true,
        ),
        Descriptor::new(
            PowFline,
            Value,
            "Line f",
            |u| if u.m.pw_fline > 0.0 { si_short(u.m.pw_fline, "Hz") } else { "auto".into() },
            |_| Vec::new(),
            |u| u.m.pw_mode == PowerMode::Ripple,
        ),
        Descriptor::new(
            PowFsw,
            Value,
            "Switch f",
            |u| if u.m.pw_fsw > 0.0 { si_short(u.m.pw_fsw, "Hz") } else { "auto".into() },
            |_| Vec::new(),
            |u| u.m.pw_mode == PowerMode::Ripple,
        ),
        Descriptor::new(
            PowRef,
            Value,
            "Reference",
            |u| u.m.pw_ref.map(|v| si_short(v, "V")).unwrap_or_else(|| "Top".into()),
            |_| Vec::new(),
            |u| u.m.pw_mode == PowerMode::Rail,
        ),
        Descriptor::new(
            PowVin,
            Choice,
            "Input",
            |u| u.m.pw_vin.map(|id| u.wave_name(id)).unwrap_or_else(|| "Off".into()),
            Ui::wave_names_opt,
            |u| matches!(u.m.pw_mode, PowerMode::Ripple | PowerMode::Rail),
        ),
        Descriptor::new(
            PowClk,
            Choice,
            "Clock",
            |u| u.m.pw_clk.map(|id| u.wave_name(id)).unwrap_or_else(|| "Off".into()),
            Ui::wave_names_opt,
            |u| u.m.pw_mode == PowerMode::Rail,
        ),
    ]
};

fn sources(ui: &Ui, menu: MenuId) -> Vec<WaveId> {
    let m = &ui.m;
    match menu {
        MenuId::Power if matches!(m.pw_mode, PowerMode::Ripple | PowerMode::Rail) => vec![m.pw_v],
        MenuId::Power => vec![m.pw_v, m.pw_i],
        _ => Vec::new(),
    }
}

fn power_page(p: PowerMode) -> &'static str {
    match p {
        PowerMode::Quality => "Quality",
        PowerMode::Harmonics => "Harmonics",
        PowerMode::Switching => "Switching",
        PowerMode::Ripple => "Ripple",
        PowerMode::Capacitance => "Cin",
        PowerMode::Rail => "Rail",
    }
}
