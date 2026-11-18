// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area = Area {
    kind: remote_kind,
    label: remote_label,
    value: Ui::remote_value,
    adjust: Ui::remote_adjust,
    options: Ui::remote_options,
    menu_items: |_, _| remote_items(),
    ..Area::EMPTY
};

/// Switches in the left column, what they open in the right one, row by row.
fn remote_items() -> Vec<Item> {
    vec![
        Item::RemWeb,
        Item::RemScpi,
        Item::RemVxi,
        Item::RemHislip,
        Item::RemTelnet,
        Item::RemSsh,
        Item::RemMdns,
        Item::RemAllow,
        Item::NetWeb,
        Item::RemScpiInfo,
        Item::RemVxiInfo,
        Item::RemHislipInfo,
        Item::RemTelnetInfo,
        Item::Open(MenuId::Ssh),
        Item::RemName,
        Item::RemFirewall,
    ]
}

pub(in crate::ui) fn remote_kind(it: Item) -> Option<Kind> {
    match it {
        Item::RemWeb
        | Item::RemScpi
        | Item::RemVxi
        | Item::RemHislip
        | Item::RemTelnet
        | Item::RemSsh
        | Item::RemMdns
        | Item::RemName
        | Item::RemAllow => Some(Kind::Choice),
        _ => Ui::ssh_kind(it),
    }
}

/// Rows that are only text.
pub(in crate::ui) fn remote_text(it: Item) -> bool {
    matches!(
        it,
        Item::NetWeb
            | Item::RemScpiInfo
            | Item::RemVxiInfo
            | Item::RemHislipInfo
            | Item::RemTelnetInfo
            | Item::RemFirewall
    ) || Ui::ssh_text(it)
}

pub(super) fn remote_label(it: Item) -> Option<&'static str> {
    Some(match it {
        Item::Open(MenuId::Remote) => "Remote services",
        Item::RemWeb => "Web remote",
        Item::RemScpi => "SCPI",
        Item::RemVxi => "VXI-11",
        Item::RemHislip => "HiSLIP",
        Item::RemTelnet => "telnet",
        Item::RemSsh => "SSH",
        Item::RemMdns => "mDNS",
        Item::RemName => "Name",
        Item::RemAllow => "Allowed",
        Item::RemFirewall => "Firewall",
        Item::NetWeb => "Address",
        Item::RemScpiInfo | Item::RemVxiInfo | Item::RemHislipInfo | Item::RemTelnetInfo => "Port",
        _ => return Ui::ssh_label(it),
    })
}
