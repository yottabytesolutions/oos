// SPDX-License-Identifier: GPL-2.0-or-later

use super::super::small_tests::{demo, read_rows, strings};
use super::*;

#[test]
fn remote_service_rows_preserve_metadata_values_and_committed_choices() {
    use Item::*;
    use Kind::*;
    let mut u = demo();
    read_rows(&u, &[(Open(MenuId::Remote), Action, "Remote services", "".into(), vec![])]);
    for on in [false, true] {
        (u.sys.remote_web, u.sys.remote_scpi, u.sys.instruments.vxi11, u.sys.instruments.hislip) = (on, on, on, on);
        (u.sys.remote_ssh, u.sys.remote_mdns) = (on, on);
        for (item, label) in [
            (RemWeb, "Web remote"),
            (RemScpi, "SCPI"),
            (RemVxi, "VXI-11"),
            (RemHislip, "HiSLIP"),
            (RemSsh, "SSH"),
            (RemMdns, "mDNS"),
        ] {
            read_rows(&u, &[(item, Choice, label, onoff(on), strings(&["Off", "On"]))]);
            assert_eq!(active_choice::index(&u, item), Some(usize::from(on)));
        }
        for (item, label, enabled) in [
            (NetWeb, "Address", "not connected"),
            (RemScpiInfo, "Port", "5555 (TCP)"),
            (RemVxiInfo, "Port", "111 and 9009 (LXI)"),
            (RemHislipInfo, "Port", "4880 (TCP)"),
        ] {
            read_rows(&u, &[(item, Text, label, if on { enabled } else { "off" }.into(), vec![])]);
            assert!(matches!(u.ctl(item, 1000), Ctl::Text));
        }
    }
}

#[test]
fn remote_name_and_access_rows_preserve_values_options_and_states() {
    use Item::*;
    use Kind::*;
    let mut u = demo();
    u.net = "192.0.2.1".into();
    assert_eq!(u.value(NetWeb), "http://192.0.2.1:8080");
    u.sys.net_name = "example".into();
    read_rows(
        &u,
        &[(
            RemName,
            Choice,
            "Name",
            "example.local".into(),
            vec![u.options(RemName)[0].clone(), "example".into(), "Other ...".into()],
        )],
    );
    u.sys.remote_mdns = false;
    assert_eq!(u.value(RemName), "example");
    assert_eq!(active_choice::index(&u, RemName), Some(1));
    u.sys.set_net_allow(Some(""));
    read_rows(
        &u,
        &[
            (RemAllow, Choice, "Allowed", "everyone".into(), strings(&["Other ..."])),
            (RemFirewall, Text, "Firewall", "off: everyone".into(), vec![]),
        ],
    );
    assert_eq!(active_choice::index(&u, RemAllow), None);
    u.sys.set_net_allow(Some("192.0.2.0/24"));
    u.remote.lans = strings(&["192.0.2.0/24", "198.51.100.0/24"]);
    assert_eq!(u.value(RemAllow), "192.0.2.0/24");
    assert_eq!(u.options(RemAllow), ["192.0.2.0/24", "198.51.100.0/24", "Other ..."]);
    assert_eq!(active_choice::index(&u, RemAllow), Some(0));
    assert_eq!(u.value(RemFirewall), "on");
    u.remote.failed = true;
    assert_eq!(u.value(RemFirewall), "failed: see the log");
    let (_sender, receiver) = std::sync::mpsc::channel();
    u.remote.job = Some(receiver);
    assert_eq!(u.value(RemFirewall), "applying");
}

#[test]
fn remote_rows_keep_their_menu_order() {
    use Item::*;
    assert_eq!(
        demo().menu_items(MenuId::Remote),
        [
            RemWeb,
            RemScpi,
            RemVxi,
            RemHislip,
            RemSsh,
            RemMdns,
            RemAllow,
            NetWeb,
            RemScpiInfo,
            RemVxiInfo,
            RemHislipInfo,
            Open(MenuId::Ssh),
            RemName,
            RemFirewall
        ]
    );
}

#[test]
fn disabled_port_rows_do_not_build_enabled_text() {
    assert_eq!(port_text(false, || panic!("disabled port text must stay lazy")), "off");
    assert_eq!(port_text(true, || "enabled".into()), "enabled");
}

#[test]
fn contexts_stay_out_of_visible_rows() {
    super::super::small_tests::contexts(MenuId::Remote, &[Item::Open(MenuId::Remote)]);
}

#[test]
fn navigation_and_visibility_at_every_text_size() {
    super::super::small_tests::navigation(MenuId::Remote, Item::RemScpi, |u, _| u.sys.remote_scpi = !u.sys.remote_scpi);
}

#[test]
fn capabilities_in_isolated_processes() {
    super::super::small_tests::capabilities(
        MenuId::Remote,
        "ui::menu::remote::tests::capabilities_in_isolated_processes",
        || {},
    );
}
