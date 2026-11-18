// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area =
    Area { descriptors: ROWS, rows, adjust: Ui::sim_adjust, activate: Ui::sim_activate, ..Area::EMPTY };

const ROWS: &[Descriptor] = &[
    TITLE,
    SIM_LIVE,
    SIM_PRESET,
    SIM_RATE,
    SIM_HIGH,
    SIM_LOW,
    SIM_RISE,
    SIM_BW,
    SIM_NOISE,
    SIM_SSC_HZ,
    SIM_DCD,
    SIM_CORR_RJ,
    SIM_CORR_HZ,
    SIM_RJ,
    SIM_PJ,
    SIM_PAT,
    SIM_DEEMPH,
    SIM_SSC,
    SIM_SSC_CENTER,
    SIM_SSC_DEVIATION,
    SIM_PJ_HZ,
    SIM_SLOT,
    SIM_PLAY,
    SIM_SHOW,
    SIM_DEFAULTS,
    ADVANCED_LINK,
];

const TITLE: Descriptor = Descriptor::action(Item::Open(MenuId::Sim), "SIM").when(|_| false);

const SIM_LIVE: Descriptor = Descriptor::action(Item::SimLive, "Live").when(Ui::record_mode);

const SIM_PRESET: Descriptor = Descriptor::choice(
    Item::SimPreset,
    "Preset",
    |u| u.sim_preset_name().into(),
    |_| oos_dsp::serial::names().into_iter().map(String::from).collect(),
);

const SIM_RATE: Descriptor = Descriptor::value(Item::SimRate, "Rate", |u| si_short(u.m.sim_rate, "b/s"));

const SIM_HIGH: Descriptor = Descriptor::value(Item::SimHigh, "High", |u| si_short(u.m.sim_vhigh, "V"));

const SIM_LOW: Descriptor = Descriptor::value(Item::SimLow, "Low", |u| si_short(u.m.sim_vlow, "V"));

const SIM_RISE: Descriptor = Descriptor::value(Item::SimRise, "Rise 20-80%", |u| si_short(u.m.sim_rise, "s"));

const SIM_BW: Descriptor = Descriptor::value(Item::SimBw, "Bandwidth", |u| si_short(u.m.sim_bw, "Hz"));

const SIM_NOISE: Descriptor = Descriptor::value(Item::SimNoise, "Noise", |u| si_short(u.m.sim_noise, "V"));

const SIM_SSC_HZ: Descriptor = Descriptor::value(Item::SimSscHz, "SSC frequency", |u| si_short(u.m.sim_ssc_hz, "Hz"));

const SIM_DCD: Descriptor = Descriptor::value(Item::SimDcd, "DCD p-p", |u| si_short(u.m.sim_dcd_pp, "s"));

const SIM_CORR_RJ: Descriptor = Descriptor::value(Item::SimCorrRj, "Corr RJ RMS", |u| si_short(u.m.sim_corr_rj, "s"));

const SIM_CORR_HZ: Descriptor =
    Descriptor::value(Item::SimCorrHz, "Corr RJ corner", |u| si_short(u.m.sim_corr_hz, "Hz"));

const SIM_RJ: Descriptor = Descriptor::value(Item::SimRj, "RJ RMS", |u| si_short(u.m.sim_rj, "s"));

const SIM_PJ: Descriptor = Descriptor::value(Item::SimPj, "PJ peak", |u| si_short(u.m.sim_pj, "s"));

const SIM_PAT: Descriptor = Descriptor::choice(
    Item::SimPat,
    "Pattern",
    |u| format!("PRBS-{}", u.m.sim_prbs),
    |_| super::super::sim::PRBS.iter().map(|n| format!("PRBS-{n}")).collect(),
);

const SIM_DEEMPH: Descriptor = Descriptor::value(Item::SimDeemph, "De-emph", |u| format!("{:.1} dB", u.m.sim_deemph));

const SIM_SSC: Descriptor = Descriptor::choice(Item::SimSsc, "Spread", |u| onoff(u.m.sim_ssc), toggles);

const SIM_SSC_CENTER: Descriptor = Descriptor::choice(
    Item::SimSscCenter,
    "SSC profile",
    |u| if u.m.sim_ssc_center { "Center" } else { "Down, DC removed" }.into(),
    |_| ["Down, DC removed", "Center"].into_iter().map(String::from).collect(),
);

const SIM_SSC_DEVIATION: Descriptor = Descriptor::value(Item::SimSscDeviation, "SSC deviation", |u| {
    format!("{:.2} % {}", 100.0 * u.m.sim_ssc_deviation, if u.m.sim_ssc_center { "pk" } else { "p-p" })
});

const SIM_PJ_HZ: Descriptor = Descriptor::value(Item::SimPjHz, "PJ freq", |u| {
    if u.m.sim_pj > 0.0 {
        si_short(u.m.sim_pjhz, "Hz")
    } else {
        "off".into()
    }
});

const SIM_SLOT: Descriptor = Descriptor::choice(
    Item::SimSlot,
    "Slot",
    |u| format!("R{}", u.m.sim_slot + 1),
    |_| (1..=NREF).map(|i| format!("R{i}")).collect(),
);

const SIM_PLAY: Descriptor = Descriptor::action(Item::SimPlay, "Play");

const SIM_SHOW: Descriptor = Descriptor::choice(
    Item::SimShow,
    "Show",
    |u| match &u.refs[u.m.sim_slot] {
        None => "empty".into(),
        Some(r) => {
            let name = if r.label.is_empty() { format!("R{}", u.m.sim_slot + 1) } else { r.label.clone() };
            format!("{} {}", name, if r.on { "on" } else { "off" })
        }
    },
    toggles,
);

const SIM_DEFAULTS: Descriptor = Descriptor::action(Item::SimDefaults, "Defaults");

const ADVANCED_LINK: Descriptor = Descriptor::action(Item::Open(MenuId::SimAdvanced), "Advanced");

const MAIN: &[Descriptor] = &[
    SIM_LIVE,
    SIM_PRESET,
    SIM_RATE,
    SIM_PAT,
    SIM_HIGH,
    SIM_LOW,
    SIM_RISE,
    SIM_BW,
    SIM_DEEMPH,
    SIM_NOISE,
    SIM_SSC,
    ADVANCED_LINK,
    SIM_RJ,
    SIM_PJ,
    SIM_PJ_HZ,
    SIM_SLOT,
    SIM_PLAY,
    SIM_SHOW,
    SIM_DEFAULTS,
];

const ADVANCED: &[Descriptor] =
    &[SIM_SSC, SIM_SSC_CENTER, SIM_SSC_DEVIATION, SIM_SSC_HZ, SIM_DCD, SIM_CORR_RJ, SIM_CORR_HZ, SIM_DEFAULTS];

fn rows(_: &Ui, menu: MenuId) -> &'static [Descriptor] {
    match menu {
        MenuId::Sim => MAIN,
        MenuId::SimAdvanced => ADVANCED,
        _ => &[],
    }
}

#[cfg(test)]
mod tests;
