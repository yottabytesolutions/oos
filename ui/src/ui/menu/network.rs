// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area = Area { descriptors: ROWS, activate, ..Area::EMPTY };

const ROWS: &[Descriptor] = {
    use Item::*;
    &[
        Descriptor::title(Open(MenuId::Network), "Network"),
        Descriptor::text(NetEth, "Ethernet", ethernet_value),
        Descriptor::reference(NetRetry10),
        Descriptor::text(NetMac, "MAC", mac_value),
        Descriptor::reference(Open(MenuId::Remote)),
        Descriptor::text(NetPorts, "Other access", |_| "SCPI 5555, VXI-11, SSH 22 (password or key)".into()),
        Descriptor::text(NetWlan, "WLAN", wlan_value),
        Descriptor::action(WlanScan, "Find WLAN networks"),
        Descriptor::action(WlanForget, "Forget WLAN network"),
        Descriptor::action(WlanInfo, "WLAN details"),
        Descriptor::picker_context(WlanNet, "Networks", Ui::wlan_net_options),
        Descriptor::picker_context(WlanForgetNet, "Forget", Ui::wlan_forget_options),
    ]
};

fn ethernet_value(ui: &Ui) -> String {
    let addr = ui.net_ifs.iter().find(|i| i.0 == "eth0").map(|i| i.1.as_str());
    let link = ui.host.text("/sys/class/net/eth0/carrier").is_some_and(|t| t.trim() == "1");
    let speed = ui.eth.speed.map_or("link up".to_string(), |s| format!("{} Mbit/s", s));
    match (addr, link) {
        (Some(a), true) => format!("{} \u{b7} {}", a, speed),
        (Some(a), false) => format!("{} \u{b7} no cable", a),
        (None, true) => "link up, no address yet".into(),
        (None, false) => "no cable".into(),
    }
}

fn mac_value(ui: &Ui) -> String {
    ui.host
        .text("/sys/class/net/eth0/address")
        .map(|t| t.trim().to_string())
        .filter(|m| !m.is_empty())
        .unwrap_or_else(|| "-".into())
}

fn wlan_value(ui: &Ui) -> String {
    let l = &ui.wlan.link;
    if !l.stick() {
        "no stick".into()
    } else if !l.ctrl() {
        "stick found, starting".into()
    } else if l.status().connected() {
        let ip = if l.status().ip.is_empty() { "waiting for an address" } else { l.status().ip.as_str() };
        format!("{} \u{b7} {}", wlan::short(&oos_hal::wlan::name(&l.status().ssid)), ip)
    } else {
        format!("not connected ({})", l.status().state.to_lowercase())
    }
}

fn activate(ui: &mut Ui, it: Item) -> bool {
    match it {
        Item::WlanInfo => ui.dialog = Some(ui.wlan_lines()),
        Item::WlanScan => ui.wlan_scan(),
        Item::WlanForget => ui.wlan_forget_tap(),
        _ => {}
    }
    true
}

#[cfg(test)]
mod tests;
