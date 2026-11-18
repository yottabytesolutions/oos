// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area =
    Area { descriptors: ROWS, adjust: Ui::afg_adjust, activate: Ui::afg_activate, ..Area::EMPTY };

const ROWS: &[Descriptor] = {
    use Item::*;
    use Kind::*;
    &[
        Descriptor::new(Open(MenuId::Afg), Action, "Generator", |_| String::new(), |_| Vec::new(), |_| false),
        Descriptor::new(
            AfgSel,
            Choice,
            "Generator",
            |u| format!("Gen {}", u.m.gen_sel + 1),
            |_| (1..=gen_count()).map(|i| format!("Gen {}", i)).collect(),
            |_| gen_count() > 1,
        ),
        Descriptor::new(AfgOn, Choice, "Output", |u| onoff(u.m.afg_on), toggles, |_| true),
        Descriptor::new(
            AfgWave,
            Choice,
            "Wave",
            |u| AFG_WAVES[u.m.afg_wave].into(),
            |_| AFG_WAVES.iter().map(|s| s.to_string()).collect(),
            |_| true,
        ),
        Descriptor::new(
            AfgFreq,
            Value,
            "Frequency",
            |u| si_short(u.m.afg_freq, "Hz"),
            |_| Vec::new(),
            |u| !matches!(u.m.afg_wave, 4 | 5),
        ),
        Descriptor::new(
            AfgAmpl,
            Value,
            "Amplitude",
            |u| si_short(u.m.afg_ampl, "Vpp"),
            |_| Vec::new(),
            |u| u.m.afg_wave != 5,
        ),
        Descriptor::new(AfgOffset, Value, "Offset", |u| si(u.m.afg_offset, "V"), |_| Vec::new(), |_| true),
        Descriptor::new(
            AfgArbSrc,
            Choice,
            "Capture from",
            |u| format!("CH{}", u.m.afg_arb_src + 1),
            |_| ch_names(),
            |u| u.m.afg_wave == 6,
        ),
        Descriptor::new(
            AfgArbGrab,
            Action,
            "Capture the screen",
            |_| String::new(),
            |_| Vec::new(),
            |u| u.m.afg_wave == 6,
        ),
        Descriptor::new(AfgArbFile, Action, "Load arb.csv", |_| String::new(), |_| Vec::new(), |u| u.m.afg_wave == 6),
        Descriptor::new(
            AfgDuty,
            Value,
            "Duty",
            |u| format!("{:.0} %", u.m.afg_duty),
            |_| Vec::new(),
            |u| matches!(u.m.afg_wave, 1 | 3),
        ),
        Descriptor::new(
            AfgSym,
            Value,
            "Symmetry",
            |u| format!("{:.0} %", u.m.afg_symmetry),
            |_| Vec::new(),
            |u| u.m.afg_wave == 2,
        ),
        Descriptor::new(
            AfgMod,
            Choice,
            "Modulation",
            |u| AFG_MODS[u.m.afg_mod].into(),
            |_| AFG_MODS.iter().map(|s| s.to_string()).collect(),
            |u| u.m.afg_wave <= 2,
        ),
        Descriptor::new(
            AfgModRate,
            Value,
            "Mod. rate",
            |u| si_short(u.m.afg_mod_rate, "Hz"),
            |_| Vec::new(),
            |u| u.m.afg_wave <= 2 && u.m.afg_mod > 0,
        ),
        Descriptor::new(
            AfgModDepth,
            Value,
            "Depth / dev.",
            |u| match u.m.afg_mod {
                2 => format!("dev {}", si_short(u.m.afg_fm_dev, "Hz")),
                3 => format!("dev {:.0}\u{b0}", u.m.afg_pm_dev),
                _ => format!("{:.0} %", u.m.afg_mod_depth),
            },
            |_| Vec::new(),
            |u| u.m.afg_wave <= 2 && u.m.afg_mod > 0,
        ),
        Descriptor::new(
            AfgModShape,
            Choice,
            "Mod. shape",
            |u| AFG_MOD_SHAPES[u.m.afg_mod_shape].into(),
            |_| AFG_MOD_SHAPES.iter().map(|s| s.to_string()).collect(),
            |u| u.m.afg_wave <= 2 && u.m.afg_mod > 0,
        ),
        Descriptor::new(
            AfgFreqEntry,
            Choice,
            "Enter frequency",
            |_| "Keypad".into(),
            |_| FREQ_KEYS.iter().map(|k| k.to_string()).collect(),
            |_| false,
        ),
    ]
};
