// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area = Area {
    descriptors: ROWS,
    label: |it| LABELS.iter().find(|r| r.0 == it).map(|r| r.1),
    ordered_rows: |_, _| ITEMS.iter().filter_map(|&it| super::descriptor(it)).collect(),
    adjust: Ui::time_adjust,
    activate: Ui::time_activate,
    ..Area::EMPTY
};

const LABELS: &[(Item, &str)] = &[(Item::Open(MenuId::Time), "Date and time")];
const ITEMS: &[Item] = {
    use Item::*;
    &[TimeNow, NtpOn, NtpServer, NtpStatus, NtpSync, TimeZone, TimeSet]
};

const ROWS: &[Descriptor] = {
    use Item::*;
    &[
        Descriptor::text(TimeNow, "Now", |u| {
            let t = u.host.unix_now() as i64;
            let z = u.zone();
            let s = format!("{} {}", z.local(t).text(), z.abbr_at(t));
            if u.clock_known {
                s
            } else {
                format!("{} (not set)", s)
            }
        }),
        Descriptor::choice(NtpOn, "Time from NTP", |u| onoff(u.sys.ntp), toggles),
        Descriptor::choice(
            NtpServer,
            "NTP server",
            |u| crate::ui::time::server_label(&u.sys.ntp_server),
            Ui::server_options,
        ),
        Descriptor::text(NtpStatus, "NTP state", Ui::ntp_status),
        Descriptor::action(NtpSync, "Sync now"),
        Descriptor::choice(TimeZone, "Time zone", Ui::zone_label, Ui::zone_options),
        Descriptor::action(TimeSet, "Set the clock"),
        Descriptor::choice(NetRetry10, "10 Mbit retry", |u| onoff(u.sys.eth_retry_10m), toggles),
    ]
};
