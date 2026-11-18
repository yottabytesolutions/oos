// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area =
    Area { descriptors: ROWS, sources, adjust: Ui::ana_adjust, activate: Ui::ana_activate, ..Area::EMPTY };

const ROWS: &[Descriptor] = {
    use Item::*;
    use Kind::*;
    &[
        Descriptor::new(Open(MenuId::Eye), Action, "Eye / jitter", |_| String::new(), |_| Vec::new(), |_| false),
        Descriptor::new(EyeAct, Action, "Auto Eye/Jitter", |_| String::new(), |_| Vec::new(), |_| true),
        Descriptor::new(EyeOn, Choice, "View", |u| onoff(u.m.view == AView::Eye), toggles, |_| true),
        Descriptor::new(
            EyePlace,
            Choice,
            "Place",
            |u| {
                match u.m.eye_place {
                    crate::model::EyePlace::Full => "Full",
                    crate::model::EyePlace::Side => "Side",
                    crate::model::EyePlace::Under => "Under",
                }
                .into()
            },
            |_| ["Full", "Side", "Under"].into_iter().map(String::from).collect(),
            |_| true,
        ),
        Descriptor::new(EyeSrc, Choice, "Source", |u| u.wave_name(u.m.eye_src), Ui::wave_names, |_| true),
        Descriptor::new(
            EyeRate,
            Value,
            "Bit rate",
            |u| {
                let m = &u.m;
                if m.eye_from_record && m.eye_rate > 0.0 {
                    format!("{} from record", si_short(m.eye_rate, "b/s"))
                } else if m.eye_rate == 0.0 {
                    "Auto".into()
                } else {
                    si_short(m.eye_rate, "b/s")
                }
            },
            |_| Vec::new(),
            |_| true,
        ),
        Descriptor::new(EyeBer, Value, "BER", |u| format!("{:.0e}", u.m.eye_ber), |_| Vec::new(), |_| true),
        Descriptor::new(
            EyeB,
            Choice,
            "Clock",
            |u| u.m.eye_b.map(|id| u.wave_name(id)).unwrap_or_else(|| "Off".into()),
            Ui::wave_names_opt,
            |_| true,
        ),
        Descriptor::new(EyeHp, Choice, "Highpass", |u| onoff(u.m.eye_hp), toggles, |_| true),
        Descriptor::new(EyePnLo, Value, "Pn low", |u| si_short(u.m.eye_pn_lo, "Hz"), |_| Vec::new(), |_| true),
        Descriptor::new(EyePnHi, Value, "Pn high", |u| si_short(u.m.eye_pn_hi, "Hz"), |_| Vec::new(), |_| true),
        Descriptor::new(EyeNpj, Choice, "NPJ", |u| onoff(u.m.eye_npj), toggles, |_| true),
        Descriptor::new(
            EyeRail,
            Choice,
            "Rail",
            |u| u.m.eye_rail.map(|id| u.wave_name(id)).unwrap_or_else(|| "Off".into()),
            Ui::wave_names_opt,
            |_| true,
        ),
        Descriptor::new(
            EyeNotch,
            Value,
            "Notch",
            |u| if u.m.eye_notch > 0.0 { si_short(u.m.eye_notch, "Hz") } else { "off".into() },
            |_| Vec::new(),
            |_| true,
        ),
        Descriptor::new(EyeClear, Action, "Clear eye", |_| String::new(), |_| Vec::new(), |_| true),
    ]
};

fn sources(ui: &Ui, menu: MenuId) -> Vec<WaveId> {
    let m = &ui.m;
    match menu {
        MenuId::Eye => vec![m.eye_src],
        _ => Vec::new(),
    }
}
