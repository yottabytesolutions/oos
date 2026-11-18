// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area =
    Area { descriptors: ROWS, rows, adjust: Ui::cal_adjust, activate: Ui::cal_activate, ..Area::EMPTY };

const CHANNELS: Descriptor = Descriptor::choice(
    Item::CalCh,
    "Channels",
    |u| if u.cal.ch == 0 { "All".into() } else { format!("CH{}", u.cal.ch) },
    |_| ["All", "CH1", "CH2", "CH3", "CH4"][..=n_ch()].iter().map(|s| (*s).into()).collect(),
);
const ADC: Descriptor = Descriptor::choice(Item::CalAdc, "ADC cores", |u| onoff(u.cal.adc), toggles);
const ZERO: Descriptor = Descriptor::choice(Item::CalZero, "AFE zero", |u| onoff(u.cal.zero), toggles);
const START: Descriptor = Descriptor::action(Item::CalStart, "Start");
const CANCEL: Descriptor = Descriptor::action(Item::CalCancel, "Cancel");
const STATUS: Descriptor = Descriptor::choice(Item::CalStatus, "Status", Ui::cal_status, |_| Vec::new());

const ROWS: &[Descriptor] = &[
    Descriptor::action(Item::Open(MenuId::SelfCal), "Self-cal").when(|_| false),
    CHANNELS,
    ADC,
    ZERO,
    START,
    CANCEL,
    STATUS,
];
const IDLE: &[Descriptor] = &[CHANNELS, ADC, ZERO, START, STATUS];
const RUNNING: &[Descriptor] = &[CHANNELS, ADC, ZERO, CANCEL, STATUS];

fn rows(ui: &Ui, _: MenuId) -> &'static [Descriptor] {
    if ui.cal_running() {
        RUNNING
    } else {
        IDLE
    }
}
