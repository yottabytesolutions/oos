// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area = Area { descriptors: ROWS, adjust: Ui::bus_adjust, activate, ..Area::EMPTY };

const ROWS: &[Descriptor] = {
    use Item::*;
    &[
        Descriptor::action(Open(MenuId::Bus), "Bus").when(|_| false),
        Descriptor::choice(BusOn, "Bus", |u| onoff(u.m.bus.on), toggles),
        Descriptor::new(BusLabel, Kind::Action, "Label", |u| u.m.bus.shown_label(), |_| Vec::new(), |_| true),
        Descriptor::value(BusPos, "Position", |u| format!("{:.1} div", u.m.bus.pos)),
        Descriptor::choice(BusClk, "Clock", |u| u.wave_name(u.m.bus.clk), sources),
        Descriptor::value(BusClkThr, "Clock threshold", |u| si(u.m.bus.clk_thr as f64, "V")),
        Descriptor::choice(BusData, "Data", |u| u.wave_name(u.m.bus.data), sources),
        Descriptor::value(BusDataThr, "Data threshold", |u| si(u.m.bus.data_thr as f64, "V")),
        Descriptor::choice(
            BusRw,
            "R/W bit",
            |u| if u.m.bus.addr_rw { "Included" } else { "Left out" }.into(),
            |_| ["Left out", "Included"].into_iter().map(String::from).collect(),
        ),
    ]
};

fn sources(ui: &Ui) -> Vec<String> {
    ui.bus_ids().iter().map(|&id| ui.wave_name(id)).collect()
}

fn activate(ui: &mut Ui, it: Item) -> bool {
    if it == Item::BusLabel {
        ui.wlan.osk = Some(wlan::Osk::for_bus(&ui.m.bus.label));
    }
    true
}
