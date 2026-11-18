// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area =
    Area { descriptors: ROWS, adjust: Ui::remote_adjust, activate: Ui::ssh_activate, ..Area::EMPTY };

const ROWS: &[Descriptor] = {
    use Item::*;
    &[
        Descriptor::title(Open(MenuId::Ssh), "SSH settings"),
        Descriptor::text(SshState, "State", |u| {
            if !u.sys.remote_ssh {
                "off".into()
            } else if u.ssh.state.is_empty() {
                "not started".into()
            } else {
                u.ssh.state.clone()
            }
        }),
        Descriptor::choice(SshPassword, "Password login", |u| onoff(u.sys.ssh_password), toggles),
        Descriptor::action(SshShowPass, "Show password"),
        Descriptor::text(SshHostKey, "Host key", |u| {
            u.ssh.host_fp.as_ref().map_or("not made yet".into(), |f| format!("{}\u{2026}", &f[..f.len().min(19)]))
        }),
        Descriptor::text(SshKeys, "Trusted keys", |u| {
            if u.ssh.keys.is_empty() {
                "none".into()
            } else {
                u.ssh.keys.len().to_string()
            }
        }),
        Descriptor::action(SshKeyList, "List keys"),
        Descriptor::action(SshImport, "Import keys from USB"),
        Descriptor::action(SshRemove, "Remove a key"),
        Descriptor::picker_context(SshRemoveKey, "Remove a key", |u| u.ssh.keys.iter().map(|k| k.label()).collect()),
    ]
};

#[cfg(test)]
mod tests;
