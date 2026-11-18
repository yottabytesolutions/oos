// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::super::bode::{BODE_MODES, BUILTIN, FREE, GENERATORS};
use super::*;

pub(super) const AREA: Area =
    Area { descriptors: ROWS, sources, adjust: Ui::bode_adjust, activate: Ui::bode_activate, ..Area::EMPTY };

const ROWS: &[Descriptor] = {
    use Item::*;
    use Kind::*;
    &[
        Descriptor::new(Open(MenuId::Bode), Action, "Bode", |_| String::new(), |_| Vec::new(), |_| false),
        Descriptor::new(BodeOn, Choice, "View", |u| onoff(u.m.view == AView::Bode), toggles, |_| true),
        Descriptor::new(
            BodeMode,
            Choice,
            "Mode",
            |u| bode_mode_name(u.m.bode_mode).into(),
            |_| BODE_MODES.iter().map(|m| bode_mode_name(*m).to_string()).collect(),
            |u| u.bode_gen() != FREE,
        ),
        Descriptor::new(BodeRun, Action, "Start / stop sweep", |_| String::new(), |_| Vec::new(), |_| true),
        Descriptor::new(BodeStart, Value, "Start freq", |u| si_short(u.m.bode_start, "Hz"), |_| Vec::new(), |_| true),
        Descriptor::new(BodeStop, Value, "Stop freq", |u| si_short(u.m.bode_stop, "Hz"), |_| Vec::new(), |_| true),
        Descriptor::new(
            BodeSwTime,
            Value,
            "Sweep time",
            |u| si_short(u.m.bode_sw_time, "s"),
            |_| Vec::new(),
            |u| u.bode_gen() == FREE,
        ),
        Descriptor::new(
            BodeSwLog,
            Choice,
            "Sweep",
            |u| if u.m.bode_sw_log { "Log" } else { "Lin" }.into(),
            |_| ["Lin", "Log"].into_iter().map(String::from).collect(),
            |u| u.bode_gen() == FREE,
        ),
        Descriptor::new(BodePoints, Value, "Points", |u| u.m.bode_points.to_string(), |_| Vec::new(), |_| true),
        Descriptor::new(
            BodeAmp,
            Value,
            "Amplitude",
            |u| si_short(u.m.bode_amp, "Vpp"),
            |_| Vec::new(),
            |u| u.bode_gen() != FREE,
        ),
        Descriptor::new(
            BodeIn,
            Choice,
            "Input (ref)",
            |u| {
                if u.bode_gen() == FREE {
                    match u.m.bode_sw_in {
                        0 => "Off".into(),
                        k => format!("CH{}", k),
                    }
                } else {
                    u.wave_name(u.m.bode_in)
                }
            },
            |u| if u.bode_gen() == FREE { opt_ch_names("Off") } else { u.wave_names() },
            |_| true,
        ),
        Descriptor::new(
            BodeRef,
            Value,
            "Ref level",
            |u| format!("{:.1} dBV", u.m.bode_ref),
            |_| Vec::new(),
            |u| u.bode_gen() == FREE && u.m.bode_sw_in == 0,
        ),
        Descriptor::new(BodeOut, Choice, "Output", |u| u.wave_name(u.m.bode_out), Ui::wave_names, |_| true),
        Descriptor::new(
            BodeGen,
            Choice,
            "Generator",
            |u| GENERATORS[u.m.bode_gen.min(GENERATORS.len() - 1)].name.into(),
            |_| GENERATORS.iter().map(|g| g.name.to_string()).collect(),
            |_| true,
        ),
        Descriptor::new(
            BodeSync,
            Choice,
            "Sync",
            |u| format!("CH{}", u.m.bode_sync + 1),
            |_| ch_names(),
            |u| u.bode_gen() == FREE,
        ),
        Descriptor::new(
            BodeSyncEdge,
            Choice,
            "Sync edge",
            |u| if u.m.bode_sync_slope == Slope::Fall { "Falling" } else { "Rising" }.into(),
            |_| ["Rising", "Falling"].into_iter().map(String::from).collect(),
            |u| u.bode_gen() == FREE,
        ),
        Descriptor::new(
            BodeSyncLvl,
            Value,
            "Sync level",
            |u| si_short(u.m.bode_sync_level as f64, "V"),
            |_| Vec::new(),
            |u| u.bode_gen() == FREE,
        ),
        Descriptor::new(BodeCur, Value, "Cursor", Ui::bode_cursor_text, |_| Vec::new(), |u| !u.bode.points.is_empty()),
        Descriptor::new(
            BodeLink,
            Choice,
            "Connection",
            |u| if u.m.bode_usb { "USB" } else { "LAN" }.into(),
            |_| ["LAN", "USB"].into_iter().map(String::from).collect(),
            |u| !matches!(u.bode_gen(), BUILTIN | FREE),
        ),
        Descriptor::new(
            BodeAddr,
            Value,
            "Gen address",
            |u| u.m.bode_addr.clone(),
            |_| Vec::new(),
            |u| !matches!(u.bode_gen(), BUILTIN | FREE) && !u.m.bode_usb,
        ),
    ]
};

fn sources(ui: &Ui, menu: MenuId) -> Vec<WaveId> {
    let m = &ui.m;
    let ch = WaveId::Ch;
    match menu {
        MenuId::Bode if m.bode_gen == super::super::bode::FREE => {
            [Some(m.bode_out), m.bode_sw_in.checked_sub(1).map(ch), Some(ch(m.bode_sync))]
                .into_iter()
                .flatten()
                .collect()
        }
        MenuId::Bode => vec![m.bode_in, m.bode_out],
        _ => Vec::new(),
    }
}

fn bode_mode_name(m: oos_dsp::bode::Mode) -> &'static str {
    match m {
        oos_dsp::bode::Mode::Loop => "Loop",
        oos_dsp::bode::Mode::Psrr => "PSRR",
        oos_dsp::bode::Mode::Z => "Z",
    }
}
