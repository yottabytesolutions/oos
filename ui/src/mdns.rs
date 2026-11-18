// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! Discovery configuration and names. mdns-sd owns DNS packets, probing and interface changes.

/// Longest name of the scope: one DNS label.
pub const NAME_MAX: usize = 63;

/// A name of the scope: one DNS label of letters, digits and hyphens (RFC 952/1123), not starting or ending with a
/// hyphen, up to 63 characters.
pub fn name_ok(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= NAME_MAX
        && s.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
        && !s.starts_with('-')
        && !s.ends_with('-')
}

/// The digits that tell this unit from others of its model: the last six of the serial number's final run of digits
/// (`DHO9A264204527` gives `204527`), else its last six letters and digits.
fn serial_tag(serial: Option<&str>) -> Option<String> {
    let s: Vec<char> = serial?.trim().chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    let digits = s.iter().rev().take_while(|c| c.is_ascii_digit()).count();
    let take = if digits > 0 { digits.min(6) } else { s.len().min(6) };
    let t: String = s[s.len() - take..].iter().collect::<String>().to_ascii_lowercase();
    (!t.is_empty()).then_some(t)
}

/// The name of a unit from its model and serial number: `dho924s-204527`; `oos` without either.
pub fn default_name(model: &str, serial: Option<&str>) -> String {
    let m: String =
        model.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect::<String>().to_ascii_lowercase();
    let mut n = match (m.is_empty(), serial_tag(serial)) {
        (true, None) => "oos".to_string(),
        (true, Some(t)) => format!("oos-{}", t),
        (false, None) => m,
        (false, Some(t)) => format!("{}-{}", m, t),
    };
    n.truncate(NAME_MAX);
    let n = n.trim_matches('-').to_string();
    if name_ok(&n) {
        n
    } else {
        "oos".into()
    }
}

/// The instance name a computer lists for the services: `Rigol DHO924S OOS (204527)` with the default name, else with
/// the name in brackets. One DNS label (63 bytes) at most.
pub fn instance_name(model: &str, serial: Option<&str>, name: &str) -> String {
    let tag = match serial_tag(serial) {
        Some(t) if name == default_name(model, serial) => t,
        _ => name.to_string(),
    };
    let mut s = format!("Rigol {} OOS ({})", model.trim(), tag);
    if s.len() > NAME_MAX {
        s = format!("OOS ({})", tag);
        s.truncate(NAME_MAX);
    }
    s
}

/// A service of the scope: `_http._tcp` and its port.
#[derive(Clone, Debug, PartialEq)]
pub struct Service {
    pub kind: String,
    pub port: u16,
    pub txt: Vec<(String, Option<String>)>,
}

/// What the responder is to announce.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Config {
    pub on: bool,
    /// the host label, `dho924s-204527` (`name_ok`)
    pub name: String,
    /// the instance label of the services, `Rigol DHO924S OOS (204527)`
    pub instance: String,
    pub services: Vec<Service>,
    /// the services listen on IPv6 too (`net6::enabled`): AAAA records for the interface's IPv6 addresses
    pub v6: bool,
}

/// What the responder does, for the page.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum State {
    #[default]
    Off,
    /// on, but no interface with an address
    NoNetwork,
    Probing,
    /// At least one service has announced; other services may still be probing.
    Running,
    /// the socket could not be opened (port 5353 taken, no multicast)
    Failed,
}

/// What the page shows: the state and the host label in use.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Status {
    pub state: State,
    pub name: String,
}

static STATUS: std::sync::Mutex<Status> = std::sync::Mutex::new(Status { state: State::Off, name: String::new() });

/// The responder's state, for the page (Off until it runs).
pub fn status() -> Status {
    oos_hal::sync::lock(&STATUS).clone()
}

pub(crate) fn set_status(s: Status) {
    *oos_hal::sync::lock(&STATUS) = s;
}

pub use crate::netio::mdns::Mdns;

#[cfg(test)]
mod tests;
