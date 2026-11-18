// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use std::ptr::null_mut;

fn address(octets: [u8; 4]) -> libc::sockaddr_in {
    // SAFETY: all-zero bytes are valid for sockaddr_in, including its padding.
    let mut address: libc::sockaddr_in = unsafe { std::mem::zeroed() };
    address.sin_family = libc::AF_INET as _;
    address.sin_addr.s_addr = u32::from_ne_bytes(octets);
    address
}

fn interface(flags: u32, address: *mut libc::sockaddr_in, mask: *mut libc::sockaddr_in) -> libc::ifaddrs {
    // SAFETY: zero flags and null pointers form a valid empty ifaddrs record.
    let mut interface: libc::ifaddrs = unsafe { std::mem::zeroed() };
    interface.ifa_flags = flags;
    interface.ifa_addr = address.cast();
    interface.ifa_netmask = mask.cast();
    interface
}

// SAFETY: callers must keep the socket addresses referenced by the records alive
// throughout this call, with AF_INET pairs stored as sockaddr_in.
unsafe fn collect(interfaces: &mut [libc::ifaddrs]) -> Vec<(Ipv4Addr, Ipv4Addr)> {
    let mut next = null_mut();
    for interface in interfaces.iter_mut().rev() {
        interface.ifa_next = next;
        next = interface;
    }
    // SAFETY: the slice owns all links until traversal returns. The caller keeps
    // the socket addresses referenced by its records alive throughout this call.
    unsafe { collect_ipv4(next) }
}

#[test]
fn null_head_returns_no_addresses() {
    // SAFETY: an empty slice has no socket addresses to keep alive.
    assert!(unsafe { collect(&mut []) }.is_empty());
}

#[test]
fn missing_addresses_and_filtered_interfaces_do_not_end_traversal() {
    let mut ip = address([192, 0, 2, 1]);
    let mut mask = address([255, 255, 255, 0]);
    let mut other_family = address([0; 4]);
    other_family.sin_family = libc::AF_UNSPEC as _;
    let up = libc::IFF_UP as u32;
    let mut interfaces = [
        interface(up, null_mut(), &raw mut mask),
        interface(up, &raw mut ip, null_mut()),
        interface(up, null_mut(), null_mut()),
        interface(up, &raw mut other_family, &raw mut mask),
        interface(up | libc::IFF_LOOPBACK as u32, &raw mut ip, &raw mut mask),
        interface(0, &raw mut ip, &raw mut mask),
        interface(up, &raw mut ip, &raw mut mask),
    ];
    // SAFETY: every socket address belongs to a local record that is still live.
    let result = unsafe { collect(&mut interfaces) };
    assert_eq!(result, [(Ipv4Addr::new(192, 0, 2, 1), Ipv4Addr::new(255, 255, 255, 0))]);
}

#[test]
fn addresses_are_copied_in_list_order_with_network_byte_order() {
    let mut first = address([192, 0, 2, 42]);
    let mut second = address([198, 51, 100, 7]);
    let mut mask = address([255, 255, 255, 0]);
    let up = libc::IFF_UP as u32;
    // SAFETY: both addresses and the mask are live local sockaddr_in records.
    let result = unsafe {
        collect(&mut [interface(up, &raw mut first, &raw mut mask), interface(up, &raw mut second, &raw mut mask)])
    };
    assert_eq!(
        result,
        [
            (Ipv4Addr::new(192, 0, 2, 42), Ipv4Addr::new(255, 255, 255, 0)),
            (Ipv4Addr::new(198, 51, 100, 7), Ipv4Addr::new(255, 255, 255, 0)),
        ]
    );
}
