// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::super::analysis::window;
use super::*;

const TRACES: [&str; 4] = ["Normal", "Average", "Max hold", "Min hold"];

pub(super) const AREA: Area =
    Area { descriptors: ROWS, sources, adjust: Ui::ana_adjust, activate: Ui::ana_activate, ..Area::EMPTY };

const ROWS: &[Descriptor] = {
    use Item::*;
    use Kind::*;
    &[
        Descriptor::new(Open(MenuId::Spectrum), Action, "Spectrum", |_| String::new(), |_| Vec::new(), |_| false),
        Descriptor::new(SpecOn, Choice, "View", |u| onoff(u.m.view == AView::Spectrum), toggles, |_| true),
        Descriptor::new(SpecSrc, Choice, "Source", |u| u.wave_name(u.m.spec_src), Ui::wave_names, |_| true),
        Descriptor::new(SpecCenter, Value, "Centre", |u| si_short(u.m.spec_center, "Hz"), |_| Vec::new(), |_| true),
        Descriptor::new(SpecSpan, Value, "Span", span, |_| Vec::new(), |_| true),
        Descriptor::new(
            SpecHarm,
            Choice,
            "Harmonics",
            |u| panes::harm_names()[u.m.spec_harm.min(PANE_HARMS.len() - 1)].clone(),
            |_| panes::harm_names(),
            |_| true,
        ),
        Descriptor::new(
            SpecRbw,
            Value,
            "RBW",
            |u| match &u.azr.spec {
                Some(s) if u.m.spec_rbw == 0.0 => format!("Auto {}", si_short(s.rbw, "Hz")),
                _ if u.m.spec_rbw == 0.0 => "Auto".into(),
                _ => si_short(u.m.spec_rbw, "Hz"),
            },
            |_| Vec::new(),
            |_| true,
        ),
        Descriptor::new(
            SpecWin,
            Choice,
            "Window",
            |u| u.m.spec_win.name().into(),
            |_| crate::math::WINDOWS.iter().map(|w| w.name().to_string()).collect(),
            |_| true,
        ),
        Descriptor::new(
            SpecTrace,
            Choice,
            "Trace",
            |u| TRACES[u.m.spec_trace.min(3)].into(),
            |_| TRACES.iter().map(|s| s.to_string()).collect(),
            |_| true,
        ),
        Descriptor::new(SpecRef, Value, "Ref level", |u| format!("{:.0} dBV", u.m.spec_ref), |_| Vec::new(), |_| true),
        Descriptor::new(SpecWf, Choice, "Waterfall", |u| onoff(u.m.spec_wf), toggles, |_| true),
        Descriptor::new(SpecDist, Choice, "THD / SNR", |u| onoff(u.m.spec_dist), toggles, |_| true),
        Descriptor::new(SpecAcprW, Value, "Ch width", |u| si_short(u.m.spec_acpr_w, "Hz"), |_| Vec::new(), |_| true),
        Descriptor::new(SpecAcprS, Value, "Ch space", |u| si_short(u.m.spec_acpr_s, "Hz"), |_| Vec::new(), |_| true),
        Descriptor::new(SpecPeak, Action, "Centre on peak", |_| String::new(), |_| Vec::new(), |_| true),
        Descriptor::new(
            SpecMargin,
            Value,
            "Limit margin",
            |u| format!("{:.0} dB", u.m.spec_margin),
            |_| Vec::new(),
            |_| true,
        ),
        Descriptor::new(SpecLimitSet, Action, "Limit from trace", |_| String::new(), |_| Vec::new(), |_| true),
        Descriptor::new(SpecLimitClear, Action, "Clear limit", |_| String::new(), |_| Vec::new(), |_| true),
    ]
};

fn sources(ui: &Ui, menu: MenuId) -> Vec<WaveId> {
    let m = &ui.m;
    match menu {
        MenuId::Spectrum => vec![m.spec_src],
        _ => Vec::new(),
    }
}

fn span(u: &Ui) -> String {
    let m = &u.m;
    if m.spec_span > 0.0 {
        si_short(window::span(u, m.spec_center, m.spec_span), "Hz")
    } else if window::follows(u) && u.azr.spec_stop > 0.0 {
        format!("Auto {}", si_short(window::span(u, u.azr.spec_stop / 2.0, u.azr.spec_stop), "Hz"))
    } else {
        "Full".into()
    }
}
