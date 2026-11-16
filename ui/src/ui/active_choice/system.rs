// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! System choices retain raw configuration identity even when their labels are shortened.
use super::{position, *};

pub(super) fn index(u: &Ui, it: Item) -> Option<usize> {
    let s = &u.sys;
    Some(match it {
        Item::Graphics => usize::from(s.gpu),
        Item::NtpOn => usize::from(s.ntp),
        Item::NetRetry10 => usize::from(s.eth_retry_10m),
        Item::RemWeb => usize::from(s.remote_web),
        Item::RemScpi => usize::from(s.remote_scpi),
        Item::RemVxi => usize::from(s.remote_vxi11),
        Item::RemTelnet => usize::from(s.remote_telnet),
        Item::RemSsh => usize::from(s.remote_ssh),
        Item::RemMdns => usize::from(s.remote_mdns),
        Item::SshPassword => usize::from(s.ssh_password),
        Item::RemName => usize::from(u.name_own().is_some()),
        Item::NtpServer => time::SERVERS.iter().position(|v| *v == s.ntp_server).unwrap_or(time::SERVERS.len()),
        Item::TimeZone => {
            if s.tz == crate::sysconf::DHCP {
                0
            } else {
                crate::clock::ZONES.iter().position(|z| z.1 == s.tz).map_or(crate::clock::ZONES.len() + 1, |i| i + 1)
            }
        }
        Item::Overclock => return position(board().clock_steps(), u.clock_next),
        Item::BootMode => usize::from(u.update_state().is_some_and(|s| s.persist)),
        Item::AuxOut => {
            return position(
                &AuxOut::choices(board().mask_hw(u.switches()) || board().zone_hw(u.switches())),
                u.m.aux_out,
            )
        }
        Item::StoreDest => return position(&crate::store::StoreDest::ALL, u.store_dest),
        Item::StoreSlot => return (u.store_slot as usize).checked_sub(1),
        Item::QuickAct => return position(&QUICKS, u.m.quick),
        Item::FeatOn(i) => usize::from(u.feature_on(features::features().get(i)?.id)),
        Item::FeatDec(i) => usize::from(u.proto_on(*PROTOS.get(i)?)),
        _ => return None,
    })
}
