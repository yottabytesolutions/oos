// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area = Area { kind, label, value, activate, options, menu_items, ..Area::EMPTY };

fn kind(it: Item) -> Option<Kind> {
    Some(match it {
        Item::WlanInfo | Item::WlanScan | Item::WlanForget => Kind::Action,
        _ => return None,
    })
}

fn label(it: Item) -> Option<&'static str> {
    Some(match it {
        Item::Open(MenuId::Network) => "Network",
        Item::WlanInfo => "WLAN details",
        Item::WlanScan => "Find WLAN networks",
        Item::WlanForget => "Forget WLAN network",
        Item::NetEth => "Ethernet",
        Item::NetMac => "MAC",
        Item::NetPorts => "Other access",
        Item::NetWlan => "WLAN",
        Item::WlanNet => "Networks",
        Item::WlanForgetNet => "Forget",
        _ => return None,
    })
}

fn value(ui: &Ui, it: Item) -> Option<String> {
    Some(match it {
        Item::NetEth | Item::NetMac | Item::NetPorts | Item::NetWlan => ui.net_value(it),
        _ => return None,
    })
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

fn options(ui: &Ui, it: Item) -> Option<Vec<String>> {
    let v: &[&str] = match it {
        Item::WlanNet => return Some(ui.wlan_net_options()),
        Item::WlanForgetNet => return Some(ui.wlan_forget_options()),
        _ => &[],
    };
    Some(v.iter().map(|s| s.to_string()).collect())
}

fn menu_items(_ui: &Ui, menu: MenuId) -> Vec<Item> {
    match menu {
        MenuId::Network => vec![
            Item::NetEth,
            Item::NetRetry10,
            Item::NetMac,
            Item::Open(MenuId::Remote),
            Item::NetPorts,
            Item::NetWlan,
            Item::WlanScan,
            Item::WlanForget,
            Item::WlanInfo,
        ],
        _ => Vec::new(),
    }
}
