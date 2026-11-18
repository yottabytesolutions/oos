// SPDX-License-Identifier: GPL-2.0-or-later

use super::super::small_tests::{demo, read_rows, strings};
use super::*;

#[test]
fn ssh_rows_keep_states_fingerprints_key_options_and_visibility() {
    use Item::*;
    use Kind::*;
    let mut u = demo();
    u.sys.remote_ssh = false;
    read_rows(
        &u,
        &[
            (Open(MenuId::Ssh), Action, "SSH settings", "".into(), vec![]),
            (SshState, Text, "State", "off".into(), vec![]),
            (SshHostKey, Text, "Host key", "not made yet".into(), vec![]),
            (SshKeys, Text, "Trusted keys", "none".into(), vec![]),
            (SshShowPass, Action, "Show password", "".into(), vec![]),
            (SshKeyList, Action, "List keys", "".into(), vec![]),
            (SshImport, Action, "Import keys from USB", "".into(), vec![]),
            (SshRemove, Action, "Remove a key", "".into(), vec![]),
            (SshRemoveKey, Choice, "Remove a key", "".into(), vec![]),
        ],
    );
    u.sys.remote_ssh = true;
    assert_eq!(u.value(SshState), "not started");
    u.ssh.state = "running".into();
    assert_eq!(u.value(SshState), "running");
    for password in [false, true] {
        u.sys.ssh_password = password;
        read_rows(&u, &[(SshPassword, Choice, "Password login", onoff(password), strings(&["Off", "On"]))]);
        assert_eq!(active_choice::index(&u, SshPassword), Some(usize::from(password)));
    }
    for (fp, expected) in [("short", "short…"), ("SHA256:abcdefghijklmnopqrstuv", "SHA256:abcdefghijkl…")] {
        u.ssh.host_fp = Some(fp.into());
        assert_eq!(u.value(SshHostKey), expected);
    }
    u.ssh.keys = crate::ssh::Keys::parse(
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIHpflgL+YU3dK6EtW+scyOzamlWiPXJSwQWA1mfwdC9N example",
    )
    .keys();
    assert_eq!(u.value(SshKeys), "1");
    assert_eq!(u.options(SshRemoveKey).len(), 1);
    assert!(u.options(SshRemoveKey)[0].starts_with("ed25519 example SHA256:"));
    assert_eq!(
        u.menu_items(MenuId::Ssh),
        [SshState, SshPassword, SshShowPass, SshHostKey, SshKeys, SshKeyList, SshImport, SshRemove]
    );
    for item in [SshState, SshHostKey, SshKeys] {
        assert!(matches!(u.ctl(item, 1000), Ctl::Text));
    }
}

#[test]
fn contexts_stay_out_of_visible_rows() {
    super::super::small_tests::contexts(MenuId::Ssh, &[Item::Open(MenuId::Ssh), Item::SshRemoveKey]);
}

#[test]
fn navigation_and_visibility_at_every_text_size() {
    super::super::small_tests::navigation(MenuId::Ssh, Item::SshPassword, |u, _| {
        u.sys.ssh_password = !u.sys.ssh_password
    });
}

#[test]
fn capabilities_in_isolated_processes() {
    super::super::small_tests::capabilities(
        MenuId::Ssh,
        "ui::menu::ssh::tests::capabilities_in_isolated_processes",
        || {},
    );
}
