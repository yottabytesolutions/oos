// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

#[test]
fn every_time_row_preserves_metadata_values_and_order() {
    use Item::*;
    use Kind::*;
    let u = demo();
    let t = u.host.unix_now() as i64;
    let z = u.zone();
    let now = format!("{} {} (not set)", z.local(t).text(), z.abbr_at(t));
    super::super::small_tests::read_rows(
        &u,
        &[
            (Open(MenuId::Time), Action, "Date and time", "".into(), vec![]),
            (TimeNow, Text, "Now", now, vec![]),
            (NtpOn, Choice, "Time from NTP", "Off".into(), strings(&["Off", "On"])),
            (
                NtpServer,
                Choice,
                "NTP server",
                "DHCP".into(),
                std::iter::once("DHCP".into())
                    .chain(crate::ui::time::SERVERS[1..].iter().map(|s| server_name(s)))
                    .chain(["Other ...".into()])
                    .collect(),
            ),
            (NtpStatus, Text, "NTP state", "Off".into(), vec![]),
            (NtpSync, Action, "Sync now", "".into(), vec![]),
            (
                TimeZone,
                Choice,
                "Time zone",
                "UTC".into(),
                std::iter::once("DHCP".into())
                    .chain(crate::clock::ZONES.iter().map(|z| z.0.into()))
                    .chain(["Other ...".into()])
                    .collect(),
            ),
            (TimeSet, Action, "Set the clock", "".into(), vec![]),
            (NetRetry10, Choice, "10 Mbit retry", onoff(u.sys.eth_retry_10m), strings(&["Off", "On"])),
        ],
    );
    assert_eq!(u.menu_items(MenuId::Time), [TimeNow, NtpOn, NtpServer, NtpStatus, NtpSync, TimeZone, TimeSet]);
}

#[test]
fn time_state_preserves_text_controls_and_toggle_identity() {
    use Item::*;
    let mut u = demo();
    for known in [false, true] {
        u.clock_known = known;
        assert_eq!(u.value(TimeNow).ends_with("(not set)"), !known);
        assert_eq!(u.value(NtpStatus), if known { "Off, clock set by hand" } else { "Off" });
        for item in [TimeNow, NtpStatus] {
            assert!(matches!(u.ctl(item, 1), Ctl::Text));
            assert_eq!(active_choice::index(&u, item), None);
        }
    }
    for on in [false, true] {
        u.sys.ntp = on;
        u.sys.eth_retry_10m = on;
        for item in [NtpOn, NetRetry10] {
            assert_eq!(u.value(item), if on { "On" } else { "Off" });
            assert_eq!(active_choice::index(&u, item), Some(usize::from(on)));
        }
        assert_eq!(u.value(NtpStatus), u.ntp_status());
    }
}

#[test]
fn time_server_and_zone_choices_preserve_labels_and_identity() {
    use Item::*;
    let mut u = demo();
    for (index, server) in crate::ui::time::SERVERS.into_iter().enumerate() {
        u.sys.ntp_server = server.into();
        assert_eq!(u.value(NtpServer), if index == 0 { "DHCP".into() } else { server_name(server) });
        assert_eq!(active_choice::index(&u, NtpServer), Some(index));
    }
    u.sys.ntp_server = "example.invalid".into();
    assert_eq!(u.value(NtpServer), "example.invalid");
    assert_eq!(&u.options(NtpServer)[crate::ui::time::SERVERS.len()..], ["example.invalid", "Other ..."]);
    for (label, zone) in crate::clock::ZONES {
        u.sys.tz = zone.into();
        assert_eq!(u.value(TimeZone), label);
    }
    u.sys.tz = "ABC-2:30".into();
    assert_eq!(u.value(TimeZone), "ABC-2:30");
    let options = u.options(TimeZone);
    assert_eq!(&options[options.len() - 2..], ["ABC-2:30", "Other ..."]);
}

fn server_name(server: &str) -> String {
    if server.len() > 18 {
        format!("{}…", &server[..17])
    } else {
        server.into()
    }
}

#[test]
fn menu_titles_are_labels_without_hidden_rows() {
    for (menu, area, label) in [
        (MenuId::Time, &super::super::time::AREA, "Date and time"),
        (MenuId::Trigger, &super::super::trigger::AREA, "Trigger"),
    ] {
        let item = Item::Open(menu);
        assert_eq!(Ui::label(item), label);
        assert!(area.descriptors.iter().all(|row| row.item != item));
    }
}
