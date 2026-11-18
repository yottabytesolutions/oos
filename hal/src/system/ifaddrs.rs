// SPDX-License-Identifier: GPL-2.0-or-later
//! Interface addresses for the dynamic LAN access policy.
use core::net::Ipv4Addr;

/// Every IPv4 address and netmask on interfaces that are up, excluding loopback.
pub fn ipv4_all() -> Vec<(Ipv4Addr, Ipv4Addr)> {
    let mut head = std::ptr::null_mut();
    // SAFETY: head is a writable pointer slot. On success getifaddrs initializes
    // it with the list owned by IfAddrs; on failure it is not read or freed.
    if unsafe { libc::getifaddrs(&mut head) } != 0 {
        return Vec::new();
    }
    let list = IfAddrs(head);
    // SAFETY: the successful getifaddrs list remains allocated until list drops.
    unsafe { collect_ipv4(list.0) }
}

struct IfAddrs(*mut libc::ifaddrs);

impl Drop for IfAddrs {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: this is the original head returned by getifaddrs. This
            // guard is its sole owner, and no references escape collect_ipv4.
            unsafe { libc::freeifaddrs(self.0) };
        }
    }
}

/// # Safety
/// Every non-null link must point to a live ifaddrs record. Its non-null address
/// and netmask must be live socket addresses, with AF_INET pairs stored as sockaddr_in.
/// All records must remain allocated and unchanged until this call returns.
unsafe fn collect_ipv4(mut current: *const libc::ifaddrs) -> Vec<(Ipv4Addr, Ipv4Addr)> {
    let mut out = Vec::new();
    // SAFETY: the caller keeps every record alive throughout traversal. as_ref
    // checks each pointer for null. Only AF_INET records are read as sockaddr_in,
    // and only copied IPv4 values leave this function.
    unsafe {
        while let Some(a) = current.as_ref() {
            current = a.ifa_next;
            let f = a.ifa_flags;
            if f & (libc::IFF_LOOPBACK as u32) != 0 || f & (libc::IFF_UP as u32) == 0 {
                continue;
            }
            let Some(address) = a.ifa_addr.as_ref() else { continue };
            if address.sa_family as i32 != libc::AF_INET {
                continue;
            }
            let Some(address) = a.ifa_addr.cast::<libc::sockaddr_in>().as_ref() else { continue };
            let Some(netmask) = a.ifa_netmask.cast::<libc::sockaddr_in>().as_ref() else { continue };
            out.push((
                Ipv4Addr::from(u32::from_be(address.sin_addr.s_addr)),
                Ipv4Addr::from(u32::from_be(netmask.sin_addr.s_addr)),
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests;
