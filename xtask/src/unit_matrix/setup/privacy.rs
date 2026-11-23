// SPDX-License-Identifier: GPL-2.0-or-later

use std::net::{IpAddr, Ipv6Addr, SocketAddr};

pub(in crate::unit_matrix) fn address_like(key: &str, value: &str) -> bool {
    let value = value.trim();
    value.split(|ch: char| ch.is_whitespace() || "[]/\\,=\"'()<>;@?#".contains(ch)).any(|token| {
        let token = token.trim_end_matches('.');
        token.parse::<IpAddr>().is_ok()
            || token.parse::<SocketAddr>().is_ok()
            || token.split_once('%').is_some_and(|(ip, _)| ip.parse::<Ipv6Addr>().is_ok())
            || mac_like(token)
            || host_like(token, key.ends_with(".label") && token == value)
    })
}

fn mac_like(token: &str) -> bool {
    let valid_parts = |separator, count, width| {
        let mut parts = token.split(separator);
        parts.clone().count() == count
            && parts.all(|part| part.len() == width && part.bytes().all(|ch| ch.is_ascii_hexdigit()))
    };
    valid_parts(':', 6, 2) || valid_parts('-', 6, 2) || valid_parts('.', 3, 4)
}

fn host_like(token: &str, free_text: bool) -> bool {
    let host = match token.rsplit_once(':') {
        Some((host, port)) if port.parse::<u16>().is_ok() => host,
        _ => token,
    };
    let dotted = host.contains('.') && host.bytes().any(|ch| ch.is_ascii_alphabetic());
    let named = host.eq_ignore_ascii_case("localhost") || host.contains('-') || free_text;
    (dotted || named) && host.parse::<f64>().is_err() && host.split('.').all(host_label)
}

fn host_label(label: &str) -> bool {
    let bytes = label.as_bytes();
    bytes.first().is_some_and(u8::is_ascii_alphanumeric)
        && bytes.last().is_some_and(u8::is_ascii_alphanumeric)
        && bytes.iter().all(|ch| ch.is_ascii_alphanumeric() || *ch == b'-')
}
