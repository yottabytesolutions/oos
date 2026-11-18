// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! WLAN through a USB stick (docs/wlan.md): wpa_supplicant response parsers and key derivation.
//! `/bin/oos-wlan` runs the driver, supplicant and DHCP; the application's netio adapter owns control IO.
//! The configuration is wpa_supplicant's own file, written by its SAVE_CONFIG.
//!
//! Only the hexadecimal forms reach wpa_supplicant (SSID bytes, the 256-bit key derived here), so no name or password
//! can break out of a command or of the saved file, and the password itself is never stored.

use crate::update::hash::sha1;

/// The interface the stick's driver creates.
pub const IFACE: &str = "wlan0";
/// wpa_supplicant's control socket of `IFACE`.
pub const CTRL: &str = "/run/wpa_supplicant/wlan0";
/// A password has 8 to 63 characters (IEEE 802.11i passphrase).
pub const PASS_LEN: (usize, usize) = (8, 63);
/// An SSID has at most 32 bytes.
pub const SSID_MAX: usize = 32;

/// How a network wants to be joined.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Security {
    Open,
    /// WPA or WPA2 personal, also a WPA2/WPA3 mixed network
    Psk,
    /// not supported here: WPA3 only, enterprise, WEP
    Other(&'static str),
}

impl Security {
    pub fn of(flags: &str) -> Security {
        if flags.contains("PSK") {
            Security::Psk
        } else if flags.contains("SAE") {
            Security::Other("WPA3")
        } else if flags.contains("EAP") {
            Security::Other("enterprise")
        } else if flags.contains("WEP") {
            Security::Other("WEP")
        } else if flags.contains("OWE") {
            Security::Other("OWE")
        } else {
            Security::Open
        }
    }
}

/// One network of a scan: the strongest access point of its name.
#[derive(Clone, Debug, PartialEq)]
pub struct Net {
    pub ssid: Vec<u8>,
    pub signal: i32,
    pub freq: u32,
    pub security: Security,
}

impl Net {
    pub fn name(&self) -> String {
        name(&self.ssid)
    }
}

/// An SSID for the screen (it is bytes, not always UTF-8).
pub fn name(ssid: &[u8]) -> String {
    String::from_utf8_lossy(ssid).chars().map(|c| if c.is_control() { '?' } else { c }).collect()
}

/// A saved network.
#[derive(Clone, Debug, PartialEq)]
pub struct Saved {
    pub id: u32,
    pub ssid: Vec<u8>,
    /// wpa_supplicant gave up on it for now: mostly a wrong password
    pub temp_disabled: bool,
    pub disabled: bool,
}

/// The answer to STATUS.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Status {
    pub state: String,
    pub ssid: Vec<u8>,
    pub ip: String,
    pub mac: String,
    pub freq: u32,
}

impl Status {
    pub fn connected(&self) -> bool {
        self.state == "COMPLETED"
    }
}

/// Undoes wpa_supplicant's `printf_encode`: SSIDs arrive with \\, \", \n, \r, \t, \e and \xNN escapes.
pub fn unescape(s: &str) -> Vec<u8> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] != b'\\' || i + 1 == b.len() {
            out.push(b[i]);
            i += 1;
            continue;
        }
        i += 2;
        match b[i - 1] {
            b'n' => out.push(b'\n'),
            b'r' => out.push(b'\r'),
            b't' => out.push(b'\t'),
            b'e' => out.push(0x1b),
            b'x' => {
                let d: Vec<u8> =
                    b[i..].iter().take(2).map_while(|c| (*c as char).to_digit(16).map(|v| v as u8)).collect();
                match d[..] {
                    [h, l] => out.push(h << 4 | l),
                    [l] => out.push(l),
                    _ => out.push(b'x'),
                }
                i += d.len();
            }
            c => out.push(c),
        }
    }
    out
}

/// SCAN_RESULTS (`bssid  frequency  signal  flags  ssid`, tab separated, a header line): one entry per name, strongest
/// first. Hidden networks (no name) are left out.
pub fn parse_scan(t: &str) -> Vec<Net> {
    let mut v: Vec<Net> = Vec::new();
    for l in t.lines().skip(1) {
        let f: Vec<&str> = l.splitn(5, '\t').collect();
        let [_, freq, signal, flags, ssid] = f[..] else { continue };
        let (Ok(freq), Ok(signal)) = (freq.parse::<u32>(), signal.parse::<i32>()) else { continue };
        let ssid = unescape(ssid);
        if ssid.is_empty() || ssid.iter().all(|c| *c == 0) || ssid.len() > SSID_MAX {
            continue;
        }
        let n = Net { ssid, signal, freq, security: Security::of(flags) };
        match v.iter_mut().find(|x| x.ssid == n.ssid) {
            Some(o) if o.signal < n.signal => *o = n,
            Some(_) => {}
            None => v.push(n),
        }
    }
    v.sort_by(|a, b| b.signal.cmp(&a.signal).then_with(|| a.ssid.cmp(&b.ssid)));
    v
}

/// LIST_NETWORKS (`id  ssid  bssid  flags`, tab separated, a header line).
pub fn parse_networks(t: &str) -> Vec<Saved> {
    t.lines()
        .skip(1)
        .filter_map(|l| {
            let f: Vec<&str> = l.splitn(4, '\t').collect();
            let id = f.first()?.parse().ok()?;
            let flags = f.get(3).copied().unwrap_or("");
            Some(Saved {
                id,
                ssid: unescape(f.get(1)?),
                temp_disabled: flags.contains("TEMP-DISABLED"),
                disabled: flags.contains("[DISABLED]"),
            })
        })
        .collect()
}

/// STATUS (`key=value` lines).
pub fn parse_status(t: &str) -> Status {
    let mut s = Status::default();
    for l in t.lines() {
        match l.split_once('=') {
            Some(("wpa_state", v)) => s.state = v.into(),
            Some(("ssid", v)) => s.ssid = unescape(v),
            Some(("ip_address", v)) => s.ip = v.into(),
            Some(("address", v)) => s.mac = v.into(),
            Some(("freq", v)) => s.freq = v.parse().unwrap_or(0),
            _ => {}
        }
    }
    s
}

/// RSSI of SIGNAL_POLL in dBm.
pub fn parse_signal(t: &str) -> Option<i32> {
    t.lines().find_map(|l| l.strip_prefix("RSSI=")?.trim().parse().ok())
}

fn hmac_sha1(key: &[u8], msg: &[&[u8]]) -> [u8; 20] {
    // a key longer than the block goes through the hash first (RFC 2104); the longest here is 63 bytes
    let mut k = [0u8; 64];
    if key.len() > 64 {
        k[..20].copy_from_slice(&sha1(&[key]));
    } else {
        k[..key.len()].copy_from_slice(key);
    }
    let ipad = k.map(|b| b ^ 0x36);
    let opad = k.map(|b| b ^ 0x5c);
    let mut parts: Vec<&[u8]> = vec![&ipad];
    parts.extend_from_slice(msg);
    let inner = sha1(&parts);
    sha1(&[&opad, &inner])
}

/// The 256-bit key of a passphrase: PBKDF2-HMAC-SHA1, 4096 rounds, the SSID as salt (IEEE 802.11i H.4).
pub fn psk(pass: &str, ssid: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    for (block, chunk) in out.chunks_mut(20).enumerate() {
        let n = (block as u32 + 1).to_be_bytes();
        let mut u = hmac_sha1(pass.as_bytes(), &[ssid, &n]);
        let mut t = u;
        for _ in 1..4096 {
            u = hmac_sha1(pass.as_bytes(), &[&u]);
            t.iter_mut().zip(u).for_each(|(a, b)| *a ^= b);
        }
        chunk.copy_from_slice(&t[..chunk.len()]);
    }
    out
}

/// What is wrong with a password (printable ASCII of 8 to 63 characters), None when it is fine.
pub fn pass_problem(p: &str) -> Option<String> {
    let n = p.chars().count();
    if n < PASS_LEN.0 || n > PASS_LEN.1 {
        return Some(format!("A password has {} to {} characters", PASS_LEN.0, PASS_LEN.1));
    }
    if !p.chars().all(|c| (' '..='~').contains(&c)) {
        return Some("Only plain ASCII characters".into());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::update::hash::hex;

    /// IEEE 802.11i H.4.3 test vectors.
    #[test]
    fn key_of_a_passphrase() {
        assert_eq!(hex(&psk("password", b"IEEE")), "f42c6fc52df0ebef9ebb4b90b38a5f902e83fe1b135a70e23aed762e9710a12e");
        assert_eq!(
            hex(&psk("ThisIsAPassword", b"ThisIsASSID")),
            "0dc0d6eb90555ed6419756b9a15ec3e3209b63df707dd508d14581f8982721af"
        );
        // longest password, longest name (the value comes from Python's hashlib)
        assert_eq!(
            hex(&psk(&format!("{}xyz", "a\"#b c".repeat(10)), &[b'Z'; 32])),
            "986b0d59e31ee983f3ddda05e5382ace06ff85570d0294cd10df55f38f7773f4"
        );
    }

    #[test]
    fn scan_list() {
        let t = "bssid / frequency / signal level / flags / ssid\n\
                 aa:bb:cc:00:00:01\t2412\t-71\t[WPA2-PSK-CCMP][ESS]\tLabor\n\
                 aa:bb:cc:00:00:02\t2437\t-48\t[WPA2-PSK-CCMP][WPS][ESS]\tLabor\n\
                 aa:bb:cc:00:00:03\t2462\t-60\t[ESS]\tGast\\x20Netz\n\
                 aa:bb:cc:00:00:04\t2462\t-30\t[WPA2-EAP-CCMP][ESS]\tFirma\n\
                 aa:bb:cc:00:00:05\t2462\t-20\t[WPA2-PSK-CCMP][ESS]\t\n\
                 aa:bb:cc:00:00:06\t2412\t-80\t[RSN-SAE-CCMP][ESS]\tNeu\n\
                 kaputt\n";
        let v = parse_scan(t);
        assert_eq!(v.iter().map(Net::name).collect::<Vec<_>>(), ["Firma", "Labor", "Gast Netz", "Neu"]);
        assert_eq!((v[1].signal, v[1].freq, v[1].security), (-48, 2437, Security::Psk));
        assert_eq!(v[0].security, Security::Other("enterprise"));
        assert_eq!(v[2].security, Security::Open);
        assert_eq!(v[3].security, Security::Other("WPA3"));
    }

    #[test]
    fn mixed_wpa2_wpa3_counts_as_psk() {
        assert_eq!(Security::of("[WPA2-PSK+SAE-CCMP][ESS]"), Security::Psk);
    }

    #[test]
    fn ssid_escapes() {
        assert_eq!(unescape(r#"a\\b\"c\n\t\e\x41\xc3\xa4"#), b"a\\b\"c\n\t\x1bA\xc3\xa4");
        // cut off by a broken peer: no panic, nothing invented
        assert_eq!(unescape("a\\"), b"a\\");
        assert_eq!(unescape(r"\xg"), b"xg");
        assert_eq!(unescape(r"\x4"), b"\x04");
        assert_eq!(name(b"a\xff\n"), "a\u{fffd}?");
    }

    #[test]
    fn saved_list() {
        let t = "network id / ssid / bssid / flags\n0\tLabor\tany\t[CURRENT]\n1\tGast\tany\t[DISABLED]\n2\tX\tany\t[TEMP-DISABLED]\nnix\n";
        let v = parse_networks(t);
        assert_eq!(v.len(), 3);
        assert!(!v[0].disabled && v[1].disabled && v[2].temp_disabled && !v[2].disabled);
        assert_eq!(v[1].ssid, b"Gast");
    }

    #[test]
    fn status_and_signal() {
        let s = parse_status("bssid=aa:bb\nfreq=2437\nssid=Labor\nid=0\nmode=station\nwpa_state=COMPLETED\nip_address=192.168.1.5\naddress=00:11:22:33:44:55\n");
        assert!(s.connected());
        assert_eq!((s.ssid.as_slice(), s.ip.as_str(), s.freq), (&b"Labor"[..], "192.168.1.5", 2437));
        assert!(!parse_status("wpa_state=SCANNING\n").connected());
        assert_eq!(parse_signal("RSSI=-54\nLINKSPEED=65\n"), Some(-54));
        assert_eq!(parse_signal("FAIL\n"), None);
    }

    #[test]
    fn passwords() {
        assert!(pass_problem("1234567").is_some());
        assert!(pass_problem("12345678").is_none());
        assert!(pass_problem(&"x".repeat(63)).is_none());
        assert!(pass_problem(&"x".repeat(64)).is_some());
        assert!(pass_problem("passwörter").is_some());
    }
}
