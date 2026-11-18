// SPDX-License-Identifier: GPL-2.0-or-later
//! Where an IPv4 packet went: its receiving interface and whether its destination was a group, from the control
//! messages of `recvmsg`. Linux and macOS send both in one IP_PKTINFO message; FreeBSD, DragonFly, OpenBSD and
//! NetBSD send the destination (IP_RECVDSTADDR) and the interface (IP_RECVIF, a `sockaddr_dl`) apart. The data is
//! read as bytes at the offsets of the `libc` structs, so a short message is ignored instead of read past.
use core::net::Ipv4Addr;
use std::mem::offset_of;

fn at<const N: usize>(data: &[u8], offset: usize) -> Option<[u8; N]> {
    data.get(offset..offset + N)?.try_into().ok()
}

/// The IPPROTO_IP socket options that ask for these messages.
#[cfg(not(any(target_os = "freebsd", target_os = "dragonfly", target_os = "openbsd", target_os = "netbsd")))]
pub(super) const OPTS: &[i32] = &[libc::IP_PKTINFO];
#[cfg(any(target_os = "freebsd", target_os = "dragonfly", target_os = "openbsd", target_os = "netbsd"))]
pub(super) const OPTS: &[i32] = &[libc::IP_RECVDSTADDR, libc::IP_RECVIF];

/// Applies one IPPROTO_IP control message of type `kind` with its `data`; other messages change nothing.
#[cfg(not(any(target_os = "freebsd", target_os = "dragonfly", target_os = "openbsd", target_os = "netbsd")))]
pub(super) fn v4(kind: i32, data: &[u8], index: &mut u32, group: &mut bool) {
    if kind != libc::IP_PKTINFO {
        return;
    }
    let i = at::<4>(data, offset_of!(libc::in_pktinfo, ipi_ifindex));
    let a = at::<4>(data, offset_of!(libc::in_pktinfo, ipi_addr));
    if let (Some(i), Some(a)) = (i, a) {
        *index = u32::from_ne_bytes(i);
        *group = Ipv4Addr::from(a).is_multicast();
    }
}

/// Applies one IPPROTO_IP control message of type `kind` with its `data`; other messages change nothing.
#[cfg(any(target_os = "freebsd", target_os = "dragonfly", target_os = "openbsd", target_os = "netbsd"))]
pub(super) fn v4(kind: i32, data: &[u8], index: &mut u32, group: &mut bool) {
    if kind == libc::IP_RECVDSTADDR {
        if let Some(a) = at::<4>(data, 0) {
            *group = Ipv4Addr::from(a).is_multicast();
        }
    } else if kind == libc::IP_RECVIF {
        if let Some(i) = at::<2>(data, offset_of!(libc::sockaddr_dl, sdl_index)) {
            *index = u32::from(u16::from_ne_bytes(i));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(any(target_os = "freebsd", target_os = "dragonfly", target_os = "openbsd", target_os = "netbsd")))]
    fn pktinfo(index: u32, dst: [u8; 4]) -> Vec<u8> {
        let mut b = vec![0u8; std::mem::size_of::<libc::in_pktinfo>()];
        let i = offset_of!(libc::in_pktinfo, ipi_ifindex);
        b[i..i + 4].copy_from_slice(&index.to_ne_bytes());
        let a = offset_of!(libc::in_pktinfo, ipi_addr);
        b[a..a + 4].copy_from_slice(&dst);
        b
    }

    #[cfg(not(any(target_os = "freebsd", target_os = "dragonfly", target_os = "openbsd", target_os = "netbsd")))]
    #[test]
    fn one_pktinfo_message_gives_interface_and_group() {
        let (mut index, mut group) = (0, true);
        v4(libc::IP_PKTINFO, &pktinfo(3, [192, 0, 2, 7]), &mut index, &mut group);
        assert_eq!((index, group), (3, false));
        v4(libc::IP_PKTINFO, &pktinfo(4, [224, 0, 0, 251]), &mut index, &mut group);
        assert_eq!((index, group), (4, true));
    }

    #[cfg(any(target_os = "freebsd", target_os = "dragonfly", target_os = "openbsd", target_os = "netbsd"))]
    #[test]
    fn two_messages_give_interface_and_group() {
        let mut dl = vec![0u8; std::mem::size_of::<libc::sockaddr_dl>()];
        let i = offset_of!(libc::sockaddr_dl, sdl_index);
        dl[i..i + 2].copy_from_slice(&5u16.to_ne_bytes());
        let (mut index, mut group) = (0, false);
        v4(libc::IP_RECVIF, &dl[..i + 2], &mut index, &mut group);
        v4(libc::IP_RECVDSTADDR, &[224, 0, 0, 251], &mut index, &mut group);
        assert_eq!((index, group), (5, true));
    }

    #[test]
    fn short_or_other_messages_change_nothing() {
        let (mut index, mut group) = (9, true);
        for &kind in OPTS {
            v4(kind, &[1], &mut index, &mut group);
        }
        v4(-1, &[0; 64], &mut index, &mut group);
        assert_eq!((index, group), (9, true));
    }
}
