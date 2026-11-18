// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area = Area { descriptors: ROWS, adjust: Ui::remote_adjust, ..Area::EMPTY };

const ROWS: &[Descriptor] = {
    use Item::*;
    &[
        Descriptor::title(Open(MenuId::Remote), "Remote services"),
        Descriptor::choice(RemWeb, "Web remote", |u| onoff(u.sys.remote_web), toggles),
        Descriptor::choice(RemScpi, "SCPI", |u| onoff(u.sys.remote_scpi), toggles),
        Descriptor::choice(RemVxi, "VXI-11", |u| onoff(u.sys.instruments.vxi11), toggles),
        Descriptor::choice(RemHislip, "HiSLIP", |u| onoff(u.sys.instruments.hislip), toggles),
        Descriptor::choice(RemSsh, "SSH", |u| onoff(u.sys.remote_ssh), toggles),
        Descriptor::choice(RemMdns, "mDNS", |u| onoff(u.sys.remote_mdns), toggles),
        Descriptor::choice(RemAllow, "Allowed", Ui::allow_value, |u| {
            u.allow_choices().into_iter().map(|c| c.0).collect()
        }),
        Descriptor::text(NetWeb, "Address", |u| {
            if !u.sys.remote_web {
                "off".into()
            } else if u.net.is_empty() {
                "not connected".into()
            } else {
                format!("http://{}:8080", u.net)
            }
        }),
        Descriptor::text(RemScpiInfo, "Port", |u| port_text(u.sys.remote_scpi, || "5555 (TCP)".into())),
        Descriptor::text(RemVxiInfo, "Port", |u| {
            port_text(u.sys.instruments.vxi11, || format!("111 and {} (LXI)", crate::vxi11::CORE_PORT))
        }),
        Descriptor::text(RemHislipInfo, "Port", |u| {
            port_text(u.sys.instruments.hislip, || format!("{} (TCP)", crate::hislip::PORT))
        }),
        Descriptor::reference(Open(MenuId::Ssh)),
        Descriptor::choice(RemName, "Name", Ui::name_value, Ui::name_options),
        Descriptor::text(RemFirewall, "Firewall", Ui::firewall_value),
    ]
};

fn port_text(on: bool, text: impl FnOnce() -> String) -> String {
    if on {
        text()
    } else {
        "off".into()
    }
}

#[cfg(test)]
mod tests;
