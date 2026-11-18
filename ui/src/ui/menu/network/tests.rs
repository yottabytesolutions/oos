// SPDX-License-Identifier: GPL-2.0-or-later

use super::super::small_tests::{demo, read_rows, strings};
use super::*;

#[test]
fn network_rows_and_picker_contexts_keep_their_read_contracts() {
    use Item::*;
    use Kind::*;
    let mut u = demo();
    read_rows(
        &u,
        &[
            (Open(MenuId::Network), Action, "Network", "".into(), vec![]),
            (NetEth, Text, "Ethernet", "no cable".into(), vec![]),
            (NetMac, Text, "MAC", "-".into(), vec![]),
            (NetPorts, Text, "Other access", "SCPI 5555, VXI-11, SSH 22 (password or key)".into(), vec![]),
            (NetWlan, Text, "WLAN", "no stick".into(), vec![]),
            (WlanScan, Action, "Find WLAN networks", "".into(), vec![]),
            (WlanForget, Action, "Forget WLAN network", "".into(), vec![]),
            (WlanInfo, Action, "WLAN details", "".into(), vec![]),
            (WlanNet, Choice, "Networks", "".into(), strings(&["Other network ..."])),
            (WlanForgetNet, Choice, "Forget", "".into(), vec![]),
        ],
    );
    for retry in [false, true] {
        u.sys.eth_retry_10m = retry;
        read_rows(&u, &[(NetRetry10, Choice, "10 Mbit retry", onoff(retry), strings(&["Off", "On"]))]);
    }
    u.host = host::Host::fixture_with_text(|p| match p {
        "/sys/class/net/eth0/carrier" => Some("1\n".into()),
        "/sys/class/net/eth0/address" => Some("00:00:5e:00:53:01\n".into()),
        _ => None,
    });
    assert_eq!(u.value(NetEth), "link up, no address yet");
    assert_eq!(u.value(NetMac), "00:00:5e:00:53:01");
    u.net_ifs.push(("eth0".into(), "192.0.2.1".into()));
    assert_eq!(u.value(NetEth), "192.0.2.1 · link up");
    u.eth.speed = Some(100);
    assert_eq!(u.value(NetEth), "192.0.2.1 · 100 Mbit/s");
    u.host = host::Host::headless();
    assert_eq!(u.value(NetEth), "192.0.2.1 · no cable");
    u.wlan.link.stick = true;
    assert_eq!(u.value(NetWlan), "stick found, starting");
    u.wlan.link.ctrl = true;
    u.wlan.link.status.state = "DISCONNECTED".into();
    assert_eq!(u.value(NetWlan), "not connected (disconnected)");
    u.wlan.link.status.state = "COMPLETED".into();
    u.wlan.link.status.ssid = b"Example".to_vec();
    assert_eq!(u.value(NetWlan), "Example · waiting for an address");
    u.wlan.link.status.ip = "192.0.2.2".into();
    assert_eq!(u.value(NetWlan), "Example · 192.0.2.2");
    assert_eq!(
        u.menu_items(MenuId::Network),
        [NetEth, NetRetry10, NetMac, Open(MenuId::Remote), NetPorts, NetWlan, WlanScan, WlanForget, WlanInfo]
    );
}

#[test]
fn wlan_context_options_keep_scan_and_saved_network_order() {
    use oos_hal::wlan::{Net, Saved, Security};
    let mut u = demo();
    for (i, security) in [Security::Open, Security::Psk, Security::Other("WPA3")].into_iter().enumerate() {
        u.wlan.nets.push(Net {
            ssid: format!("Example {i}").into_bytes(),
            signal: -40 - i as i32,
            freq: 2412,
            security,
        });
    }
    assert_eq!(
        u.options(Item::WlanNet),
        ["Example 0  -40 dBm · open", "Example 1  -41 dBm", "Example 2  -42 dBm · WPA3", "Other network ..."]
    );
    for i in 0..20 {
        u.wlan.link.saved.push(Saved {
            id: i,
            ssid: format!("Example {i}").into_bytes(),
            temp_disabled: false,
            disabled: false,
        });
    }
    assert_eq!(u.options(Item::WlanForgetNet), (0..18).map(|i| format!("Example {i}")).collect::<Vec<_>>());
    assert_eq!(
        u.options(Item::WlanNet),
        ["Example 0  -40 dBm · saved", "Example 1  -41 dBm · saved", "Example 2  -42 dBm · saved", "Other network ..."]
    );
    assert!(!u.menu_items(MenuId::Network).contains(&Item::WlanNet));
    assert!(!u.menu_items(MenuId::Network).contains(&Item::WlanForgetNet));
}

#[test]
fn mac_rows_trim_sysfs_text_and_fall_back_for_blank_or_missing_addresses() {
    let mut u = demo();
    let readers: [fn(&str) -> Option<String>; 4] =
        [|_| None, |_| Some("".into()), |_| Some(" \n\t".into()), |_| Some(" 00:00:5e:00:53:01\n".into())];
    for (reader, expected) in readers.into_iter().zip(["-", "-", "-", "00:00:5e:00:53:01"]) {
        u.host = host::Host::fixture_with_text(reader);
        assert_eq!(u.value(Item::NetMac), expected);
    }
}

#[test]
fn contexts_stay_out_of_visible_rows() {
    super::super::small_tests::contexts(
        MenuId::Network,
        &[Item::Open(MenuId::Network), Item::WlanNet, Item::WlanForgetNet],
    );
}

#[test]
fn navigation_and_visibility_at_every_text_size() {
    super::super::small_tests::navigation(MenuId::Network, Item::NetRetry10, |u, _| {
        u.sys.eth_retry_10m = !u.sys.eth_retry_10m
    });
}

#[test]
fn capabilities_in_isolated_processes() {
    super::super::small_tests::capabilities(
        MenuId::Network,
        "ui::menu::network::tests::capabilities_in_isolated_processes",
        || {},
    );
}
