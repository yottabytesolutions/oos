// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! The scope by name on the local network: a Multicast DNS responder (RFC 6762) with DNS service discovery (RFC 6763).
//! `http://dho924s-204527.local:8080` reaches the web remote, also when the scope only has a link-local address
//! (169.254.x.y, rcS without DHCP), and a computer that browses for web pages, SCPI or VXI-11 instruments lists it.
//!
//! - Answers: A of `<name>.local` (no AAAA: the services listen on IPv4 only) with the address of the interface the question came in on, the reverse
//!   PTR of its IPv4 address, `_services._dns-sd._udp.local`, and per service that is switched on a PTR, SRV and TXT
//!   (`_http._tcp` 8080, `_scpi-raw._tcp` 5555, `_vxi-11._tcp` 111). NSEC says which types a name of the scope has.
//! - Questions with the unicast-response bit (QU) get a unicast answer; a question from a port other than 5353 (a plain
//!   DNS resolver: `dig @224.0.0.251 -p 5353 ...`) gets a legacy unicast answer with its id, its questions, TTL 10.
//! - Probing (3 queries 250 ms apart) and announcing (2 answers 1 s apart) before the scope answers, after each change of
//!   the name, the services or the interfaces. A name another host holds becomes `<name>-2`, `-3` ...; an instance
//!   name `... (2)`. Goodbye (TTL 0) for what goes away, and for everything when switched off.
//! - Shares the netio runtime with the remote services. Interfaces are checked every 2 s; sockets close while off.
//!
//! The protocol is in `Responder` (no sockets: the tests drive it with packets and times); `Mdns` runs it on the network.

use core::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

pub const PORT: u16 = 5353;
pub const GROUP4: Ipv4Addr = Ipv4Addr::new(224, 0, 0, 251);
pub const GROUP6: Ipv6Addr = Ipv6Addr::new(0xff02, 0, 0, 0, 0, 0, 0, 0xfb);
/// Longest name of the scope: one DNS label.
pub const NAME_MAX: usize = 63;

const T_A: u16 = 1;
const T_PTR: u16 = 12;
const T_TXT: u16 = 16;
const T_AAAA: u16 = 28;
const T_SRV: u16 = 33;
const T_NSEC: u16 = 47;
const T_ANY: u16 = 255;
const CLASS_IN: u16 = 1;
/// The top bit of a class: cache flush in a record, unicast response (QU) in a question.
const TOP: u16 = 0x8000;
/// QR and AA: an authoritative answer.
const RESPONSE: u16 = 0x8400;
/// TTL of the records that name a host or its address (RFC 6762 §10), and of the others.
const TTL_HOST: u32 = 120;
const TTL_OTHER: u32 = 4500;
/// Most a legacy unicast answer gives (RFC 6762 §6.7).
const TTL_LEGACY: u32 = 10;
/// Payload of one packet: an Ethernet frame without the IPv6 and UDP headers, with room.
const MAX_PKT: usize = 1400;
const PROBES: u32 = 3;
const PROBE_GAP: f64 = 0.25;
const ANNOUNCES: u32 = 2;
const ANNOUNCE_GAP: f64 = 1.0;

// ------------------------------------------------------------------------------------------------ the scope's names

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

// ------------------------------------------------------------------------------------------------ names on the wire

/// A domain name as its labels.
pub type Name = Vec<String>;

fn dname(parts: &[&str]) -> Name {
    parts.iter().map(|s| s.to_string()).collect()
}

/// Names compare without regard to ASCII case (RFC 6762 §16).
fn same(a: &[String], b: &[String]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.eq_ignore_ascii_case(y))
}

fn label_bytes(l: &str) -> &[u8] {
    let b = l.as_bytes();
    &b[..b.len().min(63)]
}

/// A name uncompressed and in lower case: the form names are compared in, also inside record data.
fn canon_name(n: &[String], out: &mut Vec<u8>) {
    for l in n {
        let b = label_bytes(l);
        out.push(b.len() as u8);
        out.extend(b.iter().map(|c| c.to_ascii_lowercase()));
    }
    out.push(0);
}

fn reverse4(a: Ipv4Addr) -> Name {
    let o = a.octets();
    let mut n: Name = o.iter().rev().map(|b| b.to_string()).collect();
    n.extend(["in-addr".to_string(), "arpa".to_string()]);
    n
}

/// FNV-1a: a fingerprint of a packet.
fn fnv(b: &[u8]) -> u64 {
    b.iter().fold(0xcbf29ce484222325u64, |h, &c| (h ^ c as u64).wrapping_mul(0x100000001b3))
}

/// The NSEC type bitmap of window 0 (RFC 4034 §4.1.2).
fn type_bitmap(types: &[u16]) -> Vec<u8> {
    let Some(&max) = types.iter().filter(|t| **t < 256).max() else { return Vec::new() };
    let n = max as usize / 8 + 1;
    let mut v = vec![0u8; 2 + n];
    v[1] = n as u8;
    for &t in types.iter().filter(|t| **t < 256) {
        v[2 + t as usize / 8] |= 0x80 >> (t % 8);
    }
    v
}

fn txt_bytes(t: &[String]) -> Vec<u8> {
    if t.is_empty() {
        // an empty TXT record is one empty string (RFC 6763 §6.1)
        return vec![0];
    }
    let mut v = Vec::new();
    for s in t {
        let b = &s.as_bytes()[..s.len().min(255)];
        v.push(b.len() as u8);
        v.extend(b);
    }
    v
}

/// The data of a record of the scope.
#[derive(Clone, Debug, PartialEq)]
pub enum Data {
    A(Ipv4Addr),
    Aaaa(Ipv6Addr),
    Ptr(Name),
    Srv { port: u16, target: Name },
    Txt(Vec<String>),
    Nsec { next: Name, types: Vec<u16> },
}

impl Data {
    fn rtype(&self) -> u16 {
        match self {
            Data::A(_) => T_A,
            Data::Aaaa(_) => T_AAAA,
            Data::Ptr(_) => T_PTR,
            Data::Srv { .. } => T_SRV,
            Data::Txt(_) => T_TXT,
            Data::Nsec { .. } => T_NSEC,
        }
    }

    /// The data uncompressed, names in lower case: compared with a received record's `Rr::canon`.
    fn canon(&self) -> Vec<u8> {
        let mut v = Vec::new();
        match self {
            Data::A(a) => v.extend(a.octets()),
            Data::Aaaa(a) => v.extend(a.octets()),
            Data::Ptr(n) => canon_name(n, &mut v),
            Data::Srv { port, target } => {
                v.extend([0, 0, 0, 0]);
                v.extend(port.to_be_bytes());
                canon_name(target, &mut v);
            }
            Data::Txt(t) => v = txt_bytes(t),
            Data::Nsec { next, types } => {
                canon_name(next, &mut v);
                v.extend(type_bitmap(types));
            }
        }
        v
    }
}

/// A record of the scope. `unique`: only the scope has records of this name and type (cache flush, probed); the
/// others are shared (PTR of a service type).
#[derive(Clone, Debug, PartialEq)]
pub struct Rec {
    pub name: Name,
    pub data: Data,
    pub ttl: u32,
    pub unique: bool,
}

impl Rec {
    fn new(name: Name, data: Data, ttl: u32, unique: bool) -> Rec {
        Rec { name, data, ttl, unique }
    }

    /// The same record as a received one (name, type, class, data; not the TTL).
    fn matches(&self, r: &Rr) -> bool {
        r.rtype == self.data.rtype() && r.class == CLASS_IN && same(&r.name, &self.name) && r.canon == self.data.canon()
    }

    fn same_rec(&self, o: &Rec) -> bool {
        same(&self.name, &o.name) && self.data.rtype() == o.data.rtype() && self.data.canon() == o.data.canon()
    }
}

// ------------------------------------------------------------------------------------------------ reading packets

/// A question.
#[derive(Clone, Debug, PartialEq)]
pub struct Q {
    pub name: Name,
    pub qtype: u16,
    /// the class without the QU bit
    pub qclass: u16,
    pub qu: bool,
}

/// A received record; names inside its data are expanded (`canon`).
#[derive(Clone, Debug, PartialEq)]
pub struct Rr {
    pub name: Name,
    pub rtype: u16,
    /// the class without the cache-flush bit
    pub class: u16,
    pub flush: bool,
    pub ttl: u32,
    pub canon: Vec<u8>,
}

/// A received message.
#[derive(Clone, Debug, PartialEq)]
pub struct Msg {
    pub id: u16,
    pub flags: u16,
    pub qs: Vec<Q>,
    pub an: Vec<Rr>,
    pub ns: Vec<Rr>,
    pub ar: Vec<Rr>,
}

impl Msg {
    pub fn is_response(&self) -> bool {
        self.flags & 0x8000 != 0
    }

    /// Opcode 0, a standard query or its answer; mDNS ignores the others (RFC 6762 §18.3).
    fn standard(&self) -> bool {
        (self.flags >> 11) & 0xf == 0
    }
}

fn u16_at(b: &[u8], p: usize) -> Option<u16> {
    Some(u16::from_be_bytes([*b.get(p)?, *b.get(p + 1)?]))
}

fn u32_at(b: &[u8], p: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(p..p + 4)?.try_into().ok()?))
}

/// The name at `start` and where the bytes after it begin. Compression pointers (RFC 1035 §4.1.4) must each point
/// before the place the previous jump went to, so a loop cannot be built; a name stays within 255 bytes.
fn read_name(b: &[u8], start: usize) -> Option<(Name, usize)> {
    let mut labels = Vec::new();
    let (mut p, mut end, mut floor, mut total) = (start, None, start, 0usize);
    loop {
        let len = *b.get(p)? as usize;
        match len & 0xc0 {
            0x00 if len == 0 => {
                return Some((labels, end.unwrap_or(p + 1)));
            }
            0x00 => {
                let l = b.get(p + 1..p + 1 + len)?;
                total += len + 1;
                if total > 254 {
                    return None;
                }
                labels.push(String::from_utf8_lossy(l).into_owned());
                p += 1 + len;
            }
            0xc0 => {
                let to = ((len & 0x3f) << 8) | *b.get(p + 1)? as usize;
                if to >= floor {
                    return None;
                }
                end.get_or_insert(p + 2);
                floor = to;
                p = to;
            }
            _ => return None,
        }
    }
}

/// The data of a record at `p..p + len` with the names in it expanded and in lower case.
fn read_rdata(b: &[u8], p: usize, len: usize, rtype: u16) -> Option<Vec<u8>> {
    let rd = b.get(p..p + len)?;
    let name_in = |at: usize, out: &mut Vec<u8>| -> Option<usize> {
        let (n, end) = read_name(b, at)?;
        if end > p + len {
            return None;
        }
        canon_name(&n, out);
        Some(end)
    };
    let mut v = Vec::new();
    match rtype {
        // PTR, NS, CNAME
        T_PTR | 2 | 5 => {
            name_in(p, &mut v)?;
        }
        T_SRV => {
            v.extend(rd.get(..6)?);
            name_in(p + 6, &mut v)?;
        }
        T_NSEC => {
            let end = name_in(p, &mut v)?;
            v.extend(&b[end..p + len]);
        }
        _ => v.extend(rd),
    }
    Some(v)
}

/// A message, or None for anything malformed or cut short (never a panic, whatever the bytes).
pub fn parse(b: &[u8]) -> Option<Msg> {
    let counts = [u16_at(b, 4)?, u16_at(b, 6)?, u16_at(b, 8)?, u16_at(b, 10)?];
    let mut m =
        Msg { id: u16_at(b, 0)?, flags: u16_at(b, 2)?, qs: Vec::new(), an: Vec::new(), ns: Vec::new(), ar: Vec::new() };
    let mut p = 12;
    for _ in 0..counts[0] {
        let (name, e) = read_name(b, p)?;
        let (qtype, class) = (u16_at(b, e)?, u16_at(b, e + 2)?);
        m.qs.push(Q { name, qtype, qclass: class & !TOP, qu: class & TOP != 0 });
        p = e + 4;
    }
    for (sec, &n) in counts[1..].iter().enumerate() {
        for _ in 0..n {
            let (name, e) = read_name(b, p)?;
            let (rtype, class, ttl, len) =
                (u16_at(b, e)?, u16_at(b, e + 2)?, u32_at(b, e + 4)?, u16_at(b, e + 8)? as usize);
            let canon = read_rdata(b, e + 10, len, rtype)?;
            let r = Rr { name, rtype, class: class & !TOP, flush: class & TOP != 0, ttl, canon };
            [&mut m.an, &mut m.ns, &mut m.ar][sec].push(r);
            p = e + 10 + len;
        }
    }
    Some(m)
}

// ------------------------------------------------------------------------------------------------ writing packets

/// A packet being written, with the names written so far for compression.
struct Wr {
    b: Vec<u8>,
    names: Vec<(Name, u16)>,
}

impl Wr {
    fn new(id: u16, flags: u16) -> Wr {
        let mut b = Vec::with_capacity(512);
        b.extend(id.to_be_bytes());
        b.extend(flags.to_be_bytes());
        b.extend([0u8; 8]);
        Wr { b, names: Vec::new() }
    }

    fn u16(&mut self, v: u16) {
        self.b.extend(v.to_be_bytes());
    }

    fn counts(&mut self, c: [usize; 4]) {
        for (i, n) in c.iter().enumerate() {
            self.b[4 + 2 * i..6 + 2 * i].copy_from_slice(&(*n as u16).to_be_bytes());
        }
    }

    fn mark(&self) -> (usize, usize) {
        (self.b.len(), self.names.len())
    }

    fn back(&mut self, m: (usize, usize)) {
        self.b.truncate(m.0);
        self.names.truncate(m.1);
    }

    /// A name, its longest suffix written before as a pointer (`compress`); every suffix written is kept for later.
    fn name(&mut self, n: &[String], compress: bool) {
        for i in 0..n.len() {
            let key: Name = n[i..].iter().map(|l| l.to_ascii_lowercase()).collect();
            if compress {
                if let Some(&(_, off)) = self.names.iter().find(|(k, _)| *k == key) {
                    self.u16(0xc000 | off);
                    return;
                }
            }
            if self.b.len() < 0x3fff {
                self.names.push((key, self.b.len() as u16));
            }
            let l = label_bytes(&n[i]);
            self.b.push(l.len() as u8);
            self.b.extend(l);
        }
        self.b.push(0);
    }

    fn question(&mut self, name: &[String], qtype: u16, class: u16) {
        self.name(name, true);
        self.u16(qtype);
        self.u16(class);
    }

    fn rec(&mut self, r: &Rec, ttl: u32, flush: bool) {
        self.name(&r.name, true);
        self.u16(r.data.rtype());
        self.u16(CLASS_IN | if flush { TOP } else { 0 });
        self.b.extend(ttl.to_be_bytes());
        let at = self.b.len();
        self.u16(0);
        match &r.data {
            Data::A(a) => self.b.extend(a.octets()),
            Data::Aaaa(a) => self.b.extend(a.octets()),
            Data::Ptr(n) => self.name(n, true),
            Data::Srv { port, target } => {
                self.b.extend([0, 0, 0, 0]);
                self.u16(*port);
                self.name(target, true);
            }
            Data::Txt(t) => self.b.extend(txt_bytes(t)),
            Data::Nsec { next, types } => {
                // the next name uncompressed: what every reader takes
                self.name(next, false);
                self.b.extend(type_bitmap(types));
            }
        }
        let n = (self.b.len() - at - 2) as u16;
        self.b[at..at + 2].copy_from_slice(&n.to_be_bytes());
    }
}

/// How a record goes into an answer: TTL and cache-flush bit.
#[derive(Clone, Copy)]
enum Style {
    /// multicast or QU answer: its TTL, cache flush on unique records
    Normal,
    /// legacy unicast: TTL at most 10, no cache flush
    Legacy,
    /// goodbye: TTL 0
    Goodbye,
}

fn style(r: &Rec, s: Style) -> (u32, bool) {
    match s {
        Style::Normal => (r.ttl, r.unique),
        Style::Legacy => (r.ttl.min(TTL_LEGACY), false),
        Style::Goodbye => (0, r.unique),
    }
}

/// Answers in as many packets as they need, `qs` repeated in each (legacy unicast), the additional records in the first
/// as far as they fit.
fn responses(id: u16, qs: &[Q], an: &[Rec], ar: &[Rec], s: Style) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    let mut i = 0;
    loop {
        let mut w = Wr::new(id, RESPONSE);
        for q in qs {
            w.question(&q.name, q.qtype, q.qclass | if q.qu { TOP } else { 0 });
        }
        let mut n_an = 0;
        while i < an.len() {
            let m = w.mark();
            let (ttl, flush) = style(&an[i], s);
            w.rec(&an[i], ttl, flush);
            if w.b.len() > MAX_PKT && n_an > 0 {
                w.back(m);
                break;
            }
            n_an += 1;
            i += 1;
        }
        let mut n_ar = 0;
        if out.is_empty() {
            for r in ar {
                let m = w.mark();
                let (ttl, flush) = style(r, s);
                w.rec(r, ttl, flush);
                if w.b.len() > MAX_PKT {
                    w.back(m);
                    break;
                }
                n_ar += 1;
            }
        }
        w.counts([qs.len(), n_an, 0, n_ar]);
        out.push(w.b);
        if i >= an.len() {
            return out;
        }
    }
}

// ------------------------------------------------------------------------------------------------ the responder

/// A service of the scope: `_http._tcp` and its port.
#[derive(Clone, Debug, PartialEq)]
pub struct Service {
    pub kind: String,
    pub port: u16,
    pub txt: Vec<String>,
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

/// An interface with its addresses (only those that are up, running and take multicast).
#[derive(Clone, Debug, PartialEq)]
pub struct Iface {
    pub index: u32,
    pub name: String,
    /// address and netmask
    pub v4: Option<(Ipv4Addr, Ipv4Addr)>,
    pub v6: Vec<Ipv6Addr>,
}

/// Where a packet goes.
#[derive(Clone, Debug, PartialEq)]
pub enum Dest {
    /// 224.0.0.251:5353 on the interface
    Group4,
    /// [ff02::fb]:5353 on the interface
    Group6,
    To(SocketAddr),
}

/// A packet to send on an interface.
#[derive(Clone, Debug, PartialEq)]
pub struct Out {
    pub iface: u32,
    pub dest: Dest,
    pub pkt: Vec<u8>,
}

/// What the responder does, for the page.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum State {
    #[default]
    Off,
    /// on, but no interface with an address
    NoNetwork,
    Probing,
    Running,
    /// the socket could not be opened (port 5353 taken, no multicast)
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase {
    Off,
    /// on without an interface
    Idle,
    Probe {
        sent: u32,
        at: f64,
    },
    Announce {
        sent: u32,
        at: f64,
    },
    Run,
}

/// Which of the scope's unique names a record is about.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Owner {
    Host,
    Instance,
}

pub struct Responder {
    cfg: Config,
    ifaces: Vec<Iface>,
    /// 1: the name as set, 2: `<name>-2` ... (after conflicts)
    host_n: u32,
    inst_n: u32,
    phase: Phase,
    /// times of the last conflicts (RFC 6762 §8.1: 15 in 10 s and it waits 5 s)
    conflicts: Vec<f64>,
    seed: u64,
    /// last multicast of a record per interface: (interface, name in lower case, type), time
    sent: Vec<((u32, String, u16), f64)>,
    /// fingerprints of the packets sent in the last seconds, and when (`note`)
    mine: Vec<(u64, f64)>,
    /// lines for the log
    pub log: Vec<String>,
}

impl Responder {
    pub fn new(seed: u64) -> Responder {
        Responder {
            cfg: Config::default(),
            ifaces: Vec::new(),
            host_n: 1,
            inst_n: 1,
            phase: Phase::Off,
            conflicts: Vec::new(),
            seed: seed | 1,
            sent: Vec::new(),
            mine: Vec::new(),
            log: Vec::new(),
        }
    }

    fn rand(&mut self) -> f64 {
        // xorshift64: enough to spread the first probe of several units over 250 ms
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        (self.seed >> 11) as f64 / (1u64 << 53) as f64
    }

    /// The host label in use: the name, after conflicts with `-2`, `-3` ...
    pub fn host_label(&self) -> String {
        if self.host_n <= 1 {
            return self.cfg.name.clone();
        }
        let suffix = format!("-{}", self.host_n);
        let mut base = self.cfg.name.clone();
        base.truncate(NAME_MAX - suffix.len());
        format!("{}{}", base.trim_end_matches('-'), suffix)
    }

    fn inst_label(&self) -> String {
        if self.inst_n <= 1 {
            return self.cfg.instance.clone();
        }
        let suffix = format!(" ({})", self.inst_n);
        let mut base = self.cfg.instance.clone();
        while base.len() + suffix.len() > NAME_MAX {
            base.pop();
        }
        format!("{}{}", base, suffix)
    }

    fn host(&self) -> Name {
        vec![self.host_label(), "local".into()]
    }

    fn service_type(kind: &str) -> Name {
        let mut n: Name = kind.split('.').map(str::to_string).collect();
        n.push("local".into());
        n
    }

    fn instance(&self, kind: &str) -> Name {
        let mut n = vec![self.inst_label()];
        n.extend(Self::service_type(kind));
        n
    }

    /// The scope's names that only it may have.
    fn unique_names(&self) -> Vec<(Name, Owner)> {
        let mut v = vec![(self.host(), Owner::Host)];
        for s in &self.cfg.services {
            v.push((self.instance(&s.kind), Owner::Instance));
        }
        v
    }

    /// Every record of the scope on an interface.
    pub fn records(&self, ifc: &Iface) -> Vec<Rec> {
        let host = self.host();
        let mut v = Vec::new();
        if let Some((a, _)) = ifc.v4 {
            v.push(Rec::new(host.clone(), Data::A(a), TTL_HOST, true));
            v.push(Rec::new(reverse4(a), Data::Ptr(host.clone()), TTL_HOST, true));
        }
        // AAAA only when the services listen on IPv6 (`Config::v6`): a computer that gets an IPv6 address tries it
        // first (curl took seconds on a Mac before it fell back to IPv4); without, questions over IPv6 get the A
        // record and an NSEC
        if self.cfg.v6 {
            v.extend(ifc.v6.iter().map(|&a| Rec::new(host.clone(), Data::Aaaa(a), TTL_HOST, true)));
        }
        for s in &self.cfg.services {
            let ty = Self::service_type(&s.kind);
            let inst = self.instance(&s.kind);
            v.push(Rec::new(
                dname(&["_services", "_dns-sd", "_udp", "local"]),
                Data::Ptr(ty.clone()),
                TTL_OTHER,
                false,
            ));
            v.push(Rec::new(ty, Data::Ptr(inst.clone()), TTL_OTHER, false));
            v.push(Rec::new(inst.clone(), Data::Srv { port: s.port, target: host.clone() }, TTL_HOST, true));
            v.push(Rec::new(inst, Data::Txt(s.txt.clone()), TTL_OTHER, true));
        }
        v
    }

    pub fn state(&self) -> State {
        match self.phase {
            Phase::Off => State::Off,
            Phase::Idle => State::NoNetwork,
            Phase::Probe { .. } => State::Probing,
            Phase::Announce { .. } | Phase::Run => State::Running,
        }
    }

    fn published(&self) -> bool {
        matches!(self.phase, Phase::Announce { .. } | Phase::Run)
    }

    /// Probing from the start: after a change, a conflict, or a lost tie.
    fn restart(&mut self, now: f64, delay: f64) {
        if !self.cfg.on {
            self.phase = Phase::Off;
            return;
        }
        if self.ifaces.is_empty() {
            self.phase = Phase::Idle;
            return;
        }
        self.conflicts.retain(|t| now - t < 10.0);
        let wait = if self.conflicts.len() >= 15 { 5.0 } else { delay + self.rand() * PROBE_GAP };
        self.phase = Phase::Probe { sent: 0, at: now + wait };
    }

    /// Goodbyes for the records of `old` (records per interface before a change) that are not there any more.
    fn goodbyes(&self, old: &[(u32, Vec<Rec>)]) -> Vec<Out> {
        let mut out = Vec::new();
        for (idx, recs) in old {
            let Some(ifc) = self.ifaces.iter().find(|i| i.index == *idx) else { continue };
            let new = if self.cfg.on { self.records(ifc) } else { Vec::new() };
            let gone: Vec<Rec> = recs.iter().filter(|r| !new.iter().any(|n| n.same_rec(r))).cloned().collect();
            if gone.is_empty() {
                continue;
            }
            for pkt in responses(0, &[], &gone, &[], Style::Goodbye) {
                for dest in Self::groups(ifc) {
                    out.push(Out { iface: ifc.index, dest, pkt: pkt.clone() });
                }
            }
        }
        out
    }

    fn snapshot(&self) -> Vec<(u32, Vec<Rec>)> {
        if !self.published() {
            return Vec::new();
        }
        self.ifaces.iter().map(|i| (i.index, self.records(i))).collect()
    }

    /// A new configuration: goodbyes for what goes away, then probing again (nothing when it is the same).
    pub fn set_config(&mut self, now: f64, cfg: Config) -> Vec<Out> {
        if cfg == self.cfg {
            return Vec::new();
        }
        let old = self.snapshot();
        if cfg.name != self.cfg.name {
            self.host_n = 1;
        }
        if cfg.instance != self.cfg.instance {
            self.inst_n = 1;
        }
        self.cfg = cfg;
        let out = self.goodbyes(&old);
        self.restart(now, 0.0);
        self.note(now, out)
    }

    /// The interfaces as they are now (sorted by index): probing again on any change. An address that went away from
    /// an interface still there gets its goodbye.
    pub fn set_ifaces(&mut self, now: f64, mut ifaces: Vec<Iface>) -> Vec<Out> {
        ifaces.sort_by_key(|i| i.index);
        if ifaces == self.ifaces {
            return Vec::new();
        }
        let old = self.snapshot();
        self.ifaces = ifaces;
        let out = self.goodbyes(&old);
        self.restart(now, 0.0);
        self.note(now, out)
    }

    fn groups(ifc: &Iface) -> Vec<Dest> {
        let mut v = Vec::new();
        if ifc.v4.is_some() {
            v.push(Dest::Group4);
        }
        if !ifc.v6.is_empty() {
            v.push(Dest::Group6);
        }
        v
    }

    /// When `tick` has something to do next.
    pub fn next_wake(&self) -> Option<f64> {
        match self.phase {
            Phase::Probe { at, .. } | Phase::Announce { at, .. } => Some(at),
            _ => None,
        }
    }

    /// Probes and announcements that are due.
    pub fn tick(&mut self, now: f64) -> Vec<Out> {
        let out = self.due(now);
        self.note(now, out)
    }

    fn due(&mut self, now: f64) -> Vec<Out> {
        let mut out = Vec::new();
        if let Phase::Probe { sent, at } = self.phase {
            if now < at {
                return out;
            }
            if sent < PROBES {
                for ifc in &self.ifaces {
                    let pkt = self.probe(ifc);
                    for dest in Self::groups(ifc) {
                        out.push(Out { iface: ifc.index, dest, pkt: pkt.clone() });
                    }
                }
                self.phase = Phase::Probe { sent: sent + 1, at: now + PROBE_GAP };
                return out;
            }
            self.phase = Phase::Announce { sent: 0, at: now };
        }
        if let Phase::Announce { sent, at } = self.phase {
            if now < at {
                return out;
            }
            for ifc in &self.ifaces {
                for pkt in responses(0, &[], &self.records(ifc), &[], Style::Normal) {
                    for dest in Self::groups(ifc) {
                        out.push(Out { iface: ifc.index, dest, pkt: pkt.clone() });
                    }
                }
            }
            if sent == 0 {
                let ifs: Vec<String> = self.ifaces.iter().map(|i| i.name.clone()).collect();
                let services: Vec<&str> = self.cfg.services.iter().map(|s| s.kind.as_str()).collect();
                self.log.push(format!(
                    "mDNS: {}.local on {}, services {}",
                    self.host_label(),
                    ifs.join(" "),
                    if services.is_empty() { "none".into() } else { services.join(" ") }
                ));
            }
            self.phase = if sent + 1 >= ANNOUNCES {
                Phase::Run
            } else {
                Phase::Announce { sent: sent + 1, at: now + ANNOUNCE_GAP }
            };
        }
        out
    }

    /// A probe (RFC 6762 §8.1): QU questions of type ANY for the unique names, the records in the authority section.
    fn probe(&self, ifc: &Iface) -> Vec<u8> {
        let mut w = Wr::new(0, 0);
        let names = self.unique_names();
        for (n, _) in &names {
            w.question(n, T_ANY, CLASS_IN | TOP);
        }
        let recs: Vec<Rec> =
            self.records(ifc).into_iter().filter(|r| r.unique && names.iter().any(|(n, _)| same(n, &r.name))).collect();
        for r in &recs {
            w.rec(r, r.ttl, false);
        }
        w.counts([names.len(), 0, recs.len(), 0]);
        w.b
    }

    /// Keeps a fingerprint of each packet sent for a few seconds: the scope's own packets come back (multicast loop, a
    /// second interface on the same network) and are neither questions nor conflicts. By the bytes, not the sender: a
    /// host's own responder (macOS) sends from the same address and port and is answered.
    fn note(&mut self, now: f64, out: Vec<Out>) -> Vec<Out> {
        self.mine.retain(|(_, t)| now - t < 5.0);
        for o in &out {
            self.mine.push((fnv(&o.pkt), now));
        }
        out
    }

    /// A unicast sender on the link of the interface (RFC 6762 §11: answers only there).
    fn on_link(ifc: &Iface, ip: IpAddr) -> bool {
        match ip {
            IpAddr::V4(a) => {
                let ll = a.octets()[0] == 169 && a.octets()[1] == 254;
                ll || ifc.v4.is_some_and(|(x, m)| u32::from(x) & u32::from(m) == u32::from(a) & u32::from(m))
            }
            IpAddr::V6(a) => {
                (a.segments()[0] & 0xffc0) == 0xfe80 || ifc.v6.iter().any(|x| x.segments()[..4] == a.segments()[..4])
            }
        }
    }

    fn conflict(&mut self, now: f64, owner: Owner, why: &str) {
        self.conflicts.push(now);
        if matches!(self.phase, Phase::Probe { .. }) {
            let old = if owner == Owner::Host {
                format!("{}.local", self.host_label())
            } else {
                format!("\"{}\"", self.inst_label())
            };
            match owner {
                Owner::Host => self.host_n += 1,
                Owner::Instance => self.inst_n += 1,
            }
            let new = if owner == Owner::Host {
                format!("{}.local", self.host_label())
            } else {
                format!("\"{}\"", self.inst_label())
            };
            self.log.push(format!("mDNS: {} is taken ({}), now {}", old, why, new));
            self.restart(now, 0.0);
        } else {
            self.log.push(format!("mDNS: another host answers for a name of the scope ({}), probing again", why));
            self.restart(now, 0.0);
        }
    }

    /// A packet from `src` on interface `iface`; `to_group`: it was sent to the multicast group (not to the scope's
    /// own address). Gives the answers to send.
    pub fn receive(&mut self, now: f64, pkt: &[u8], src: SocketAddr, iface: u32, to_group: bool) -> Vec<Out> {
        let Some(m) = parse(pkt) else { return Vec::new() };
        // a packet of the host itself can come in on loopback (macOS): then the interface of its address
        let by_addr = |i: &&Iface| match src.ip() {
            IpAddr::V4(a) => i.v4.is_some_and(|(x, m)| u32::from(x) & u32::from(m) == u32::from(a) & u32::from(m)),
            IpAddr::V6(a) => i.v6.contains(&a),
        };
        let Some(ifc) =
            self.ifaces.iter().find(|i| i.index == iface).or_else(|| self.ifaces.iter().find(by_addr)).cloned()
        else {
            return Vec::new();
        };
        if !self.cfg.on || !m.standard() {
            return Vec::new();
        }
        if self.mine.iter().any(|(h, _)| *h == fnv(pkt)) {
            return Vec::new();
        }
        if m.is_response() {
            // answers come from port 5353 (RFC 6762 §11)
            if src.port() == PORT {
                self.check_answers(now, &m);
            }
            return Vec::new();
        }
        if !m.ns.is_empty() && matches!(self.phase, Phase::Probe { .. }) {
            self.tie_break(now, &m, &ifc);
        }
        if !self.published() || (!to_group && !Self::on_link(&ifc, src.ip())) {
            return Vec::new();
        }
        let out = self.answer(now, &m, src, &ifc, to_group);
        self.note(now, out)
    }

    /// A response from another host with a record of one of the scope's unique names that the scope does not have:
    /// a conflict (RFC 6762 §8.1 while probing, §9 after).
    fn check_answers(&mut self, now: f64, m: &Msg) {
        if matches!(self.phase, Phase::Off | Phase::Idle) {
            return;
        }
        let ours: Vec<Rec> = self.ifaces.iter().flat_map(|i| self.records(i)).collect();
        for r in m.an.iter().chain(&m.ar) {
            if r.ttl == 0 || r.class != CLASS_IN {
                continue;
            }
            let Some((_, owner)) = self.unique_names().into_iter().find(|(n, _)| same(n, &r.name)) else { continue };
            if ours.iter().any(|o| o.matches(r)) {
                continue;
            }
            let why = format!("{} type {}", r.name.join("."), r.rtype);
            return self.conflict(now, owner, &why);
        }
    }

    /// Two hosts probing for one name at once (RFC 6762 §8.2): the records of the authority sections compared, class,
    /// type and data in that order; the lower set loses and probes again after a second.
    fn tie_break(&mut self, now: f64, m: &Msg, ifc: &Iface) {
        let recs = self.records(ifc);
        for (name, _) in self.unique_names() {
            let mut theirs: Vec<(u16, u16, Vec<u8>)> =
                m.ns.iter().filter(|r| same(&r.name, &name)).map(|r| (r.class, r.rtype, r.canon.clone())).collect();
            if theirs.is_empty() {
                continue;
            }
            let mut ours: Vec<(u16, u16, Vec<u8>)> = recs
                .iter()
                .filter(|r| r.unique && same(&r.name, &name))
                .map(|r| (CLASS_IN, r.data.rtype(), r.data.canon()))
                .collect();
            theirs.sort();
            ours.sort();
            if ours < theirs {
                self.log.push(format!(
                    "mDNS: another host probes for {} too and wins the tie, probing again in 1 s",
                    name.join(".")
                ));
                self.conflicts.push(now);
                self.phase = Phase::Probe { sent: 0, at: now + 1.0 };
                return;
            }
        }
    }

    /// The answers to a query (RFC 6762 §6).
    fn answer(&mut self, now: f64, m: &Msg, src: SocketAddr, ifc: &Iface, to_group: bool) -> Vec<Out> {
        let recs = self.records(ifc);
        let uniq = self.unique_names();
        let mut an: Vec<Rec> = Vec::new();
        let mut all_qu = true;
        for q in &m.qs {
            if q.qclass != CLASS_IN && q.qclass != T_ANY {
                continue;
            }
            let hits: Vec<&Rec> = recs
                .iter()
                .filter(|r| same(&r.name, &q.name) && (q.qtype == T_ANY || q.qtype == r.data.rtype()))
                .collect();
            if hits.is_empty() {
                // a name of the scope without that type: say which types it has (§6.1)
                if uniq.iter().any(|(n, _)| same(n, &q.name)) {
                    let n = Self::nsec(&recs, &q.name);
                    if !an.iter().any(|a| a.same_rec(&n)) {
                        an.push(n);
                    }
                } else {
                    continue;
                }
            }
            all_qu &= q.qu;
            for r in hits {
                if !an.iter().any(|a| a.same_rec(r)) {
                    an.push(r.clone());
                }
            }
        }
        // known answers (§7.1): what the asker has with at least half the TTL is left out
        an.retain(|r| !m.an.iter().any(|k| r.matches(k) && k.ttl >= r.ttl / 2));
        if an.is_empty() {
            return Vec::new();
        }
        let ar = self.additional(&recs, &an);
        let legacy = src.port() != PORT;
        if legacy {
            return responses(m.id, &m.qs, &an, &ar, Style::Legacy)
                .into_iter()
                .map(|pkt| Out { iface: ifc.index, dest: Dest::To(src), pkt })
                .collect();
        }
        if all_qu || !to_group {
            return responses(0, &[], &an, &ar, Style::Normal)
                .into_iter()
                .map(|pkt| Out { iface: ifc.index, dest: Dest::To(src), pkt })
                .collect();
        }
        // multicast: a record sent in the last second is not sent again, except to defend against a probe (§6)
        self.sent.retain(|(_, t)| now - t < 1.0);
        if m.ns.is_empty() {
            an.retain(|r| {
                !self.sent.iter().any(|((i, n, t), _)| {
                    *i == ifc.index && *t == r.data.rtype() && *n == r.name.join(".").to_ascii_lowercase()
                })
            });
            if an.is_empty() {
                return Vec::new();
            }
        }
        for r in &an {
            self.sent.push(((ifc.index, r.name.join(".").to_ascii_lowercase(), r.data.rtype()), now));
        }
        let dest = if src.is_ipv4() { Dest::Group4 } else { Dest::Group6 };
        responses(0, &[], &an, &ar, Style::Normal)
            .into_iter()
            .map(|pkt| Out { iface: ifc.index, dest: dest.clone(), pkt })
            .collect()
    }

    /// The NSEC record of a name: the types the scope has for it.
    fn nsec(recs: &[Rec], name: &[String]) -> Rec {
        let mut types: Vec<u16> = recs.iter().filter(|r| same(&r.name, name)).map(|r| r.data.rtype()).collect();
        types.sort();
        types.dedup();
        Rec::new(name.to_vec(), Data::Nsec { next: name.to_vec(), types }, TTL_HOST, true)
    }

    /// What a resolver will ask next (RFC 6763 §12, RFC 6762 §6.2): for a service PTR its SRV, TXT and the host's
    /// addresses; for an SRV the addresses; for an address the other family's (or an NSEC when there is none).
    fn additional(&self, recs: &[Rec], an: &[Rec]) -> Vec<Rec> {
        let host = self.host();
        let mut want: Vec<Rec> = Vec::new();
        let addrs = |want: &mut Vec<Rec>| {
            want.extend(
                recs.iter().filter(|r| same(&r.name, &host) && matches!(r.data, Data::A(_) | Data::Aaaa(_))).cloned(),
            );
        };
        for a in an {
            match &a.data {
                Data::Ptr(target) => {
                    let inst: Vec<&Rec> = recs
                        .iter()
                        .filter(|r| same(&r.name, target) && matches!(r.data, Data::Srv { .. } | Data::Txt(_)))
                        .collect();
                    if !inst.is_empty() {
                        want.extend(inst.into_iter().cloned());
                        addrs(&mut want);
                    }
                }
                Data::Srv { .. } => addrs(&mut want),
                Data::A(_) | Data::Aaaa(_) => addrs(&mut want),
                _ => {}
            }
        }
        if want.iter().chain(an).any(|r| matches!(r.data, Data::A(_)))
            && !recs.iter().any(|r| matches!(r.data, Data::Aaaa(_)))
        {
            want.push(Self::nsec(recs, &host));
        }
        let mut out: Vec<Rec> = Vec::new();
        for r in want {
            if !an.iter().chain(out.iter()).any(|o| o.same_rec(&r)) {
                out.push(r);
            }
        }
        out
    }
}

impl Responder {
    pub(crate) fn stop(&mut self, now: f64) -> Vec<Out> {
        let mut cfg = self.cfg.clone();
        cfg.on = false;
        self.set_config(now, cfg)
    }

    pub(crate) fn flush_log(&mut self) {
        for line in self.log.drain(..) {
            eprintln!("oos-ui: {}", line);
        }
    }
}

// ------------------------------------------------------------------------------------------------ on the network

/// What the page shows: the state and the host label in use.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Status {
    pub state: State,
    pub name: String,
}

static STATUS: std::sync::Mutex<Status> = std::sync::Mutex::new(Status { state: State::Off, name: String::new() });

/// Every IPv4 address with its netmask of the interfaces that are up (not loopback), in the order the kernel lists
/// them; an interface can have several (a DHCP or set address and a link-local one).
pub fn ipv4_networks() -> Vec<(Ipv4Addr, Ipv4Addr)> {
    crate::netio::mdns::ipv4_all()
}

/// The responder's state, for the page (Off until it runs).
pub fn status() -> Status {
    STATUS.lock().map(|s| s.clone()).unwrap_or_default()
}

pub(crate) fn set_status(s: Status) {
    if let Ok(mut g) = STATUS.lock() {
        *g = s;
    }
}

pub use crate::netio::mdns::Mdns;

#[cfg(test)]
mod tests {
    use super::*;

    const ETH: u32 = 2;

    fn eth(a: [u8; 4]) -> Iface {
        Iface {
            index: ETH,
            name: "eth0".into(),
            v4: Some((Ipv4Addr::from(a), Ipv4Addr::new(255, 255, 255, 0))),
            v6: Vec::new(),
        }
    }

    fn cfg() -> Config {
        Config {
            on: true,
            name: "dho924s-204527".into(),
            instance: "Rigol DHO924S OOS (204527)".into(),
            services: vec![
                Service { kind: "_http._tcp".into(), port: 8080, txt: vec!["path=/".into()] },
                Service { kind: "_scpi-raw._tcp".into(), port: 5555, txt: Vec::new() },
                Service { kind: "_vxi-11._tcp".into(), port: 111, txt: Vec::new() },
            ],
            v6: false,
        }
    }

    /// A responder through probing and announcing on eth0 at 192.168.1.20.
    fn running() -> Responder {
        let mut r = Responder::new(7);
        r.set_config(0.0, cfg());
        r.set_ifaces(0.0, vec![eth([192, 168, 1, 20])]);
        let mut t = 0.0;
        while r.phase != Phase::Run {
            t += 0.05;
            r.tick(t);
            assert!(t < 5.0);
        }
        r
    }

    fn query(id: u16, qs: &[(&[&str], u16, bool)]) -> Vec<u8> {
        let mut w = Wr::new(id, 0);
        for (n, t, qu) in qs {
            w.question(&dname(n), *t, CLASS_IN | if *qu { TOP } else { 0 });
        }
        w.counts([qs.len(), 0, 0, 0]);
        w.b
    }

    fn peer(port: u16) -> SocketAddr {
        SocketAddr::from(([192, 168, 1, 50], port))
    }

    fn one(out: &[Out]) -> Msg {
        assert_eq!(out.len(), 1, "{:?}", out);
        parse(&out[0].pkt).expect("our own packet parses")
    }

    fn has(rs: &[Rr], name: &str, t: u16) -> bool {
        rs.iter().any(|r| r.rtype == t && r.name.join(".").eq_ignore_ascii_case(name))
    }

    #[test]
    fn names_of_the_scope() {
        for s in ["dho924s-204527", "a", "lab-3", &"x".repeat(63)] {
            assert!(name_ok(s), "{}", s);
        }
        for s in ["", "-a", "a-", "a b", "a.b", "ä", &"x".repeat(64), "a_b"] {
            assert!(!name_ok(s), "{}", s);
        }
        assert_eq!(default_name("DHO924S", Some("DHO9A264204527")), "dho924s-204527");
        assert_eq!(default_name("DHO804", Some("DHO8A000000001")), "dho804-000001");
        assert_eq!(default_name("DHO924S", None), "dho924s");
        assert_eq!(default_name("DHO924S", Some("ABCDEFGH")), "dho924s-cdefgh");
        assert_eq!(default_name("", None), "oos");
        assert_eq!(default_name("DHO 924/S", Some(" 12 ")), "dho924s-12");
        assert!(name_ok(&default_name(&"M".repeat(80), Some("1234567"))));
        assert_eq!(instance_name("DHO924S", Some("DHO9A264204527"), "dho924s-204527"), "Rigol DHO924S OOS (204527)");
        assert_eq!(instance_name("DHO924S", Some("DHO9A264204527"), "lab-scope-3"), "Rigol DHO924S OOS (lab-scope-3)");
        assert_eq!(instance_name("DHO924S", None, "dho924s"), "Rigol DHO924S OOS (dho924s)");
        assert!(instance_name("DHO924S", None, &"x".repeat(63)).len() <= NAME_MAX);
    }

    #[test]
    fn compressed_names_are_read_and_loops_refused() {
        // a question "a.local" at 12, then a record "b" + pointer to "local" (offset 14)
        let mut b = vec![0, 1, 0, 0, 0, 1, 0, 1, 0, 0, 0, 0];
        b.extend([1, b'a', 5, b'l', b'o', b'c', b'a', b'l', 0, 0, 1, 0, 1]);
        b.extend([1, b'b', 0xc0, 14, 0, 1, 0, 1, 0, 0, 0, 120, 0, 4, 10, 0, 0, 1]);
        let m = parse(&b).unwrap();
        assert_eq!(m.qs[0].name, dname(&["a", "local"]));
        assert_eq!(m.an[0].name, dname(&["b", "local"]));
        assert_eq!(m.an[0].canon, vec![10, 0, 0, 1]);
        // a pointer to itself, one forward, two pointing at each other
        let head = vec![0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0];
        for name in [vec![0xc0, 12], vec![0xc0, 14, 0], vec![1, b'x', 0xc0, 12], vec![0xc0, 14, 0xc0, 12]] {
            let mut p = head.clone();
            p.extend(&name);
            p.extend([0, 1, 0, 1]);
            assert!(parse(&p).is_none(), "{:?}", name);
        }
        // 0x40 and 0x80 label types, a name over 255 bytes
        for bad in [0x40u8, 0x80] {
            let mut p = head.clone();
            p.extend([bad, 0, 0, 1, 0, 1]);
            assert!(parse(&p).is_none());
        }
        let mut p = head.clone();
        for _ in 0..5 {
            p.push(60);
            p.extend([b'x'; 60]);
        }
        p.extend([0, 0, 1, 0, 1]);
        assert!(parse(&p).is_none());
        // names in record data: a PTR whose target is compressed
        let mut w = Wr::new(0, RESPONSE);
        let r = Rec::new(
            dname(&["_http", "_tcp", "local"]),
            Data::Ptr(dname(&["Scope", "_http", "_tcp", "local"])),
            4500,
            false,
        );
        w.rec(&r, 4500, false);
        w.counts([0, 1, 0, 0]);
        assert_eq!(w.b.len(), 12 + 18 + 10 + 8, "the target points back");
        let m = parse(&w.b).unwrap();
        assert!(r.matches(&m.an[0]));
    }

    /// Malformed bytes never panic: every cut of real packets, random bytes, and random changes of real packets.
    #[test]
    fn malformed_packets_never_panic() {
        let mut r = running();
        let mut seeds: Vec<Vec<u8>> = vec![r.probe(&eth([10, 0, 0, 1]))];
        let q = query(1, &[(&["_http", "_tcp", "local"], T_PTR, false)]);
        seeds.extend(r.receive(1.0, &q, peer(PORT), ETH, true).into_iter().map(|o| o.pkt));
        seeds.extend(responses(0, &[], &r.records(&eth([10, 0, 0, 1])), &[], Style::Normal));
        seeds.push(q);
        for s in &seeds {
            for n in 0..s.len() {
                let _ = parse(&s[..n]);
                let _ = r.receive(2.0, &s[..n], peer(PORT), ETH, true);
            }
        }
        let mut x = 0x9e3779b97f4a7c15u64;
        let mut rnd = || {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x
        };
        for i in 0..20_000 {
            let mut p = if i % 2 == 0 {
                (0..(rnd() % 300)).map(|_| rnd() as u8).collect::<Vec<u8>>()
            } else {
                seeds[i % seeds.len()].clone()
            };
            if !p.is_empty() {
                for _ in 0..1 + rnd() % 4 {
                    let k = (rnd() as usize) % p.len();
                    p[k] = rnd() as u8;
                }
            }
            let _ = parse(&p);
            let _ = r.receive(3.0, &p, peer(if i % 3 == 0 { 40000 } else { PORT }), ETH, i % 5 != 0);
        }
    }

    #[test]
    fn probes_then_announces_then_answers() {
        let mut r = Responder::new(3);
        r.set_config(0.0, cfg());
        assert_eq!(r.state(), State::NoNetwork);
        r.set_ifaces(0.0, vec![eth([169, 254, 7, 9])]);
        assert_eq!(r.state(), State::Probing);
        let q = query(0, &[(&["dho924s-204527", "local"], T_A, false)]);
        let (mut probes, mut ann) = (Vec::new(), Vec::new());
        let mut t = 0.0;
        while r.phase != Phase::Run {
            for o in r.tick(t) {
                assert_eq!(o.dest, Dest::Group4);
                let m = parse(&o.pkt).unwrap();
                if m.is_response() { &mut ann } else { &mut probes }.push((t, m));
            }
            // no answers while probing
            if matches!(r.phase, Phase::Probe { .. }) {
                assert!(r.receive(t, &q, peer(PORT), ETH, true).is_empty());
            }
            t += 0.01;
        }
        assert_eq!(probes.len(), 3);
        assert!(probes[0].0 <= 0.25 + 1e-9);
        for w in probes.windows(2) {
            assert!((w[1].0 - w[0].0 - 0.25).abs() < 0.011, "{} {}", w[0].0, w[1].0);
        }
        let p = &probes[0].1;
        assert!(!p.is_response());
        assert!(p.qs.iter().all(|q| q.qtype == T_ANY && q.qu));
        assert!(p.qs.iter().any(|q| q.name == dname(&["dho924s-204527", "local"])));
        assert!(p.qs.iter().any(|q| q.name.join(".") == "Rigol DHO924S OOS (204527)._http._tcp.local"));
        assert!(
            has(&p.ns, "dho924s-204527.local", T_A)
                && has(&p.ns, "Rigol DHO924S OOS (204527)._scpi-raw._tcp.local", T_SRV)
        );
        // announcements: two, a second apart, every record with cache flush on the unique ones
        assert_eq!(ann.len(), 2);
        assert!(ann[0].0 - probes[2].0 >= 0.25 - 1e-9);
        assert!((ann[1].0 - ann[0].0 - 1.0).abs() < 0.011);
        let a = &ann[0].1;
        assert!(a.is_response() && a.flags & 0x0400 != 0);
        let addr = a.an.iter().find(|r| r.rtype == T_A).unwrap();
        assert!(addr.flush && addr.ttl == 120 && addr.canon == vec![169, 254, 7, 9]);
        assert!(a
            .an
            .iter()
            .any(|r| r.rtype == T_PTR && !r.flush && r.name.join(".") == "_services._dns-sd._udp.local"));
        assert!(has(&a.an, "9.7.254.169.in-addr.arpa", T_PTR));
        assert_eq!(r.log.len(), 1, "{:?}", r.log);
        assert!(r.log[0].contains("dho924s-204527.local on eth0"));
        // now it answers
        assert!(!r.receive(t, &q, peer(PORT), ETH, true).is_empty());
        assert_eq!(r.next_wake(), None);
    }

    #[test]
    fn answers_a_ptr_srv_txt() {
        let mut r = running();
        // A, multicast, with the cache-flush bit and an NSEC for the missing AAAA
        let m = one(&r.receive(10.0, &query(0, &[(&["DHO924S-204527", "local"], T_A, false)]), peer(PORT), ETH, true));
        assert_eq!((m.id, m.qs.len()), (0, 0));
        assert_eq!(m.an.len(), 1);
        assert!(m.an[0].flush && m.an[0].canon == vec![192, 168, 1, 20]);
        assert!(m.ar.iter().any(|r| r.rtype == T_NSEC));
        // the same again within a second: not multicast twice
        assert!(r
            .receive(10.5, &query(0, &[(&["dho924s-204527", "local"], T_A, false)]), peer(PORT), ETH, true)
            .is_empty());
        // browsing: the PTR with SRV, TXT and the address in the additional section
        let m = one(&r.receive(11.0, &query(0, &[(&["_http", "_tcp", "local"], T_PTR, false)]), peer(PORT), ETH, true));
        assert_eq!(m.an.len(), 1);
        let want = Rec::new(
            dname(&["_http", "_tcp", "local"]),
            Data::Ptr(dname(&["Rigol DHO924S OOS (204527)", "_http", "_tcp", "local"])),
            4500,
            false,
        );
        assert!(want.matches(&m.an[0]) && !m.an[0].flush);
        let inst = "Rigol DHO924S OOS (204527)._http._tcp.local";
        assert!(has(&m.ar, inst, T_SRV) && has(&m.ar, inst, T_TXT) && has(&m.ar, "dho924s-204527.local", T_A));
        let srv = m.ar.iter().find(|r| r.rtype == T_SRV).unwrap();
        let mut c = vec![0, 0, 0, 0, 0x1f, 0x90];
        canon_name(&dname(&["dho924s-204527", "local"]), &mut c);
        assert_eq!(srv.canon, c, "port 8080, the host as target");
        let txt = m.ar.iter().find(|r| r.rtype == T_TXT).unwrap();
        assert_eq!(txt.canon, b"\x06path=/".to_vec());
        // SRV and TXT asked directly; an empty TXT is one empty string
        let m = one(&r.receive(
            12.0,
            &query(0, &[(&["Rigol DHO924S OOS (204527)", "_scpi-raw", "_tcp", "local"], T_ANY, false)]),
            peer(PORT),
            ETH,
            true,
        ));
        assert!(has(&m.an, "Rigol DHO924S OOS (204527)._scpi-raw._tcp.local", T_SRV));
        assert_eq!(m.an.iter().find(|r| r.rtype == T_TXT).unwrap().canon, vec![0]);
        // the service types
        let m = one(&r.receive(
            13.0,
            &query(0, &[(&["_services", "_dns-sd", "_udp", "local"], T_PTR, false)]),
            peer(PORT),
            ETH,
            true,
        ));
        assert_eq!(m.an.len(), 3);
        // the reverse name
        let m = one(&r.receive(
            14.0,
            &query(0, &[(&["20", "1", "168", "192", "in-addr", "arpa"], T_PTR, false)]),
            peer(PORT),
            ETH,
            true,
        ));
        assert_eq!(m.an[0].canon[1..15].to_vec(), b"dho924s-204527".to_vec());
        // AAAA of a host without IPv6: NSEC that lists A only
        let m =
            one(&r.receive(15.0, &query(0, &[(&["dho924s-204527", "local"], T_AAAA, false)]), peer(PORT), ETH, true));
        assert_eq!(m.an[0].rtype, T_NSEC);
        assert_eq!(m.an[0].canon[m.an[0].canon.len() - 3..].to_vec(), vec![0, 1, 0x40]);
        // other names and other hosts' services: nothing
        assert!(r.receive(16.0, &query(0, &[(&["other", "local"], T_A, false)]), peer(PORT), ETH, true).is_empty());
        assert!(r
            .receive(16.0, &query(0, &[(&["_ipp", "_tcp", "local"], T_PTR, false)]), peer(PORT), ETH, true)
            .is_empty());
        // an interface the responder does not know: by the sender's network (the host's own query on loopback), else
        // nothing
        let q = query(9, &[(&["dho924s-204527", "local"], T_A, false)]);
        assert!(r.receive(17.0, &q, SocketAddr::from(([10, 9, 9, 9], 40000)), 99, true).is_empty());
        let out = r.receive(17.0, &q, SocketAddr::from(([192, 168, 1, 20], 40000)), 1, true);
        assert_eq!(one(&out).an[0].canon, vec![192, 168, 1, 20]);
        assert_eq!(out[0].iface, ETH);
    }

    #[test]
    fn qu_and_legacy_unicast() {
        let mut r = running();
        // QU: to the asker, port 5353
        let out = r.receive(10.0, &query(0, &[(&["dho924s-204527", "local"], T_A, true)]), peer(PORT), ETH, true);
        assert_eq!(out[0].dest, Dest::To(peer(PORT)));
        assert!(parse(&out[0].pkt).unwrap().an[0].flush);
        // a plain resolver (dig): its id, its question, TTL 10 at most, no cache flush, to its port
        let out =
            r.receive(10.1, &query(0x4242, &[(&["_http", "_tcp", "local"], T_PTR, false)]), peer(53000), ETH, true);
        assert_eq!(out[0].dest, Dest::To(peer(53000)));
        let m = one(&out);
        assert_eq!(m.id, 0x4242);
        assert_eq!(m.qs.len(), 1);
        assert_eq!(m.qs[0].name, dname(&["_http", "_tcp", "local"]));
        assert!(m.an.iter().chain(&m.ar).all(|r| r.ttl <= 10 && !r.flush));
        // again at once: a legacy query is always answered
        assert_eq!(
            r.receive(10.2, &query(0x4243, &[(&["dho924s-204527", "local"], T_A, false)]), peer(53000), ETH, true)
                .len(),
            1
        );
        // sent to the scope's own address: only from the same link
        let q = query(1, &[(&["dho924s-204527", "local"], T_A, false)]);
        assert_eq!(r.receive(11.0, &q, peer(PORT), ETH, false)[0].dest, Dest::To(peer(PORT)));
        assert!(r.receive(11.0, &q, SocketAddr::from(([8, 8, 8, 8], PORT)), ETH, false).is_empty());
        assert!(!r.receive(11.0, &q, SocketAddr::from(([169, 254, 3, 4], 40000)), ETH, false).is_empty());
    }

    #[test]
    fn known_answers_are_left_out() {
        let mut r = running();
        let mut w = Wr::new(0, 0);
        w.question(&dname(&["_services", "_dns-sd", "_udp", "local"]), T_PTR, CLASS_IN);
        let ty = dname(&["_services", "_dns-sd", "_udp", "local"]);
        w.rec(&Rec::new(ty.clone(), Data::Ptr(dname(&["_http", "_tcp", "local"])), 4500, false), 3000, false);
        w.rec(&Rec::new(ty.clone(), Data::Ptr(dname(&["_scpi-raw", "_tcp", "local"])), 4500, false), 100, false);
        w.counts([1, 2, 0, 0]);
        let m = one(&r.receive(10.0, &w.b, peer(PORT), ETH, true));
        // _http known with more than half the TTL, _scpi-raw with too little
        assert_eq!(m.an.len(), 2);
        assert!(!m.an.iter().any(|a| a.canon.starts_with(b"\x05_http")));
    }

    fn response_from(name: &[&str], data: Data) -> Vec<u8> {
        let mut w = Wr::new(0, RESPONSE);
        w.rec(&Rec::new(dname(name), data, 120, true), 120, true);
        w.counts([0, 1, 0, 0]);
        w.b
    }

    #[test]
    fn a_taken_name_gets_a_number() {
        let mut r = Responder::new(5);
        r.set_config(0.0, cfg());
        r.set_ifaces(0.0, vec![eth([192, 168, 1, 20])]);
        r.tick(0.3);
        // the scope's own probe and announcement come back: no conflict
        let own = SocketAddr::from(([192, 168, 1, 20], PORT));
        for o in r.tick(0.6) {
            r.receive(0.6, &o.pkt, own, ETH, true);
        }
        let same = response_from(&["dho924s-204527", "local"], Data::A(Ipv4Addr::new(192, 168, 1, 20)));
        r.receive(0.6, &same, peer(PORT), ETH, true);
        assert_eq!(r.host_label(), "dho924s-204527");
        // a response from another host on port 40000 does not count
        let other = response_from(&["dho924s-204527", "local"], Data::A(Ipv4Addr::new(192, 168, 1, 77)));
        r.receive(0.7, &other, peer(40000), ETH, true);
        assert_eq!(r.host_label(), "dho924s-204527");
        // another host answers the probe: -2, and probing again
        r.receive(0.7, &other, peer(PORT), ETH, true);
        assert_eq!(r.host_label(), "dho924s-204527-2");
        assert!(matches!(r.phase, Phase::Probe { sent: 0, .. }));
        assert!(r.log.last().unwrap().contains("dho924s-204527.local is taken"), "{:?}", r.log);
        let other2 = response_from(&["DHO924S-204527-2", "local"], Data::A(Ipv4Addr::new(192, 168, 1, 78)));
        r.tick(1.0);
        r.receive(1.0, &other2, peer(PORT), ETH, true);
        assert_eq!(r.host_label(), "dho924s-204527-3");
        // an instance name: (2)
        let srv = response_from(
            &["Rigol DHO924S OOS (204527)", "_http", "_tcp", "local"],
            Data::Srv { port: 80, target: dname(&["x", "local"]) },
        );
        r.receive(1.1, &srv, peer(PORT), ETH, true);
        assert_eq!(r.inst_label(), "Rigol DHO924S OOS (204527) (2)");
        // probes and announcements under the new names
        let mut t = 1.1;
        while r.phase != Phase::Run {
            t += 0.05;
            r.tick(t);
        }
        let m =
            one(&r.receive(t + 2.0, &query(0, &[(&["dho924s-204527-3", "local"], T_A, false)]), peer(PORT), ETH, true));
        assert_eq!(m.an[0].canon, vec![192, 168, 1, 20]);
        assert!(r
            .receive(t + 2.0, &query(0, &[(&["dho924s-204527", "local"], T_A, false)]), peer(PORT), ETH, true)
            .is_empty());
        // a new name set by hand starts without a number
        let mut c = cfg();
        c.name = "lab-3".into();
        r.set_config(t + 3.0, c);
        assert_eq!(r.host_label(), "lab-3");
    }

    #[test]
    fn a_conflict_after_announcing_probes_again() {
        let mut r = running();
        let other = response_from(&["dho924s-204527", "local"], Data::A(Ipv4Addr::new(192, 168, 1, 77)));
        r.receive(20.0, &other, peer(PORT), ETH, true);
        // not renamed yet: the probe finds out whether the other host still holds the name
        assert_eq!(r.host_label(), "dho924s-204527");
        assert_eq!(r.state(), State::Probing);
        // a goodbye of another host for the name is no conflict
        let mut r = running();
        let mut w = Wr::new(0, RESPONSE);
        w.rec(
            &Rec::new(dname(&["dho924s-204527", "local"]), Data::A(Ipv4Addr::new(192, 168, 1, 77)), 120, true),
            0,
            true,
        );
        w.counts([0, 1, 0, 0]);
        r.receive(20.0, &w.b, peer(PORT), ETH, true);
        assert_eq!(r.state(), State::Running);
    }

    #[test]
    fn simultaneous_probes_are_broken_by_the_data() {
        let probe_with = |a: [u8; 4]| {
            let mut w = Wr::new(0, 0);
            w.question(&dname(&["dho924s-204527", "local"]), T_ANY, CLASS_IN | TOP);
            w.rec(&Rec::new(dname(&["dho924s-204527", "local"]), Data::A(Ipv4Addr::from(a)), 120, true), 120, false);
            w.counts([1, 0, 1, 0]);
            w.b
        };
        let mut r = Responder::new(9);
        r.set_config(0.0, cfg());
        r.set_ifaces(0.0, vec![eth([192, 168, 1, 20])]);
        r.tick(0.3);
        // the other host's address is lower: the scope wins and goes on
        r.receive(0.4, &probe_with([192, 168, 1, 10]), peer(PORT), ETH, true);
        assert!(matches!(r.phase, Phase::Probe { sent: 1, .. }));
        // higher: the scope loses, waits a second and probes again
        r.receive(0.4, &probe_with([192, 168, 1, 30]), peer(PORT), ETH, true);
        assert!(matches!(r.phase, Phase::Probe { sent: 0, at } if (at - 1.4).abs() < 1e-9));
        assert_eq!(r.host_label(), "dho924s-204527");
    }

    #[test]
    fn goodbyes_when_a_service_or_everything_goes() {
        let mut r = running();
        let mut c = cfg();
        c.services.retain(|s| s.kind != "_vxi-11._tcp");
        let out = r.set_config(30.0, c.clone());
        let m = one(&out);
        assert!(m.an.iter().all(|a| a.ttl == 0));
        assert!(has(&m.an, "_services._dns-sd._udp.local", T_PTR) && has(&m.an, "_vxi-11._tcp.local", T_PTR));
        assert!(!has(&m.an, "_http._tcp.local", T_PTR) && !has(&m.an, "dho924s-204527.local", T_A));
        assert_eq!(r.state(), State::Probing);
        // switched off: every record with TTL 0, then nothing more
        let mut t = 30.0;
        while r.phase != Phase::Run {
            t += 0.05;
            r.tick(t);
        }
        c.on = false;
        let m = one(&r.set_config(t, c));
        assert!(has(&m.an, "dho924s-204527.local", T_A) && m.an.iter().all(|a| a.ttl == 0));
        assert_eq!(r.state(), State::Off);
        assert!(r.tick(t + 10.0).is_empty());
        assert!(r
            .receive(t, &query(0, &[(&["dho924s-204527", "local"], T_A, false)]), peer(PORT), ETH, true)
            .is_empty());
    }

    #[test]
    fn a_new_address_is_probed_and_announced() {
        let mut r = Responder::new(11);
        r.set_config(0.0, cfg());
        r.set_ifaces(0.0, vec![eth([169, 254, 7, 9])]);
        let mut t = 0.0;
        while r.phase != Phase::Run {
            t += 0.05;
            r.tick(t);
        }
        // the same list again: nothing
        assert!(r.set_ifaces(t, vec![eth([169, 254, 7, 9])]).is_empty());
        assert_eq!(r.state(), State::Running);
        // DHCP after the link-local address: a goodbye for the old address, then probing
        let m = one(&r.set_ifaces(t, vec![eth([192, 168, 1, 20])]));
        assert!(m.an.iter().all(|a| a.ttl == 0) && m.an.iter().any(|a| a.canon == vec![169, 254, 7, 9]));
        assert_eq!(r.state(), State::Probing);
        // a WLAN stick: both interfaces, each with its own address
        let mut w = eth([10, 0, 0, 5]);
        (w.index, w.name) = (5, "wlan0".into());
        w.v6 = vec!["fe80::1234".parse().unwrap()];
        r.set_ifaces(t, vec![w, eth([192, 168, 1, 20])]);
        let mut outs = Vec::new();
        while r.phase != Phase::Run {
            t += 0.05;
            outs.extend(r.tick(t));
        }
        for (i, d) in [(5, Dest::Group4), (5, Dest::Group6), (ETH, Dest::Group4)] {
            assert!(outs.iter().any(|o| o.iface == i && o.dest == d), "{} {:?}", i, d);
        }
        assert!(!outs.iter().any(|o| o.iface == ETH && o.dest == Dest::Group6), "eth0 has no IPv6 address");
        let m = one(&r.receive(
            t + 2.0,
            &query(0, &[(&["dho924s-204527", "local"], T_ANY, false)]),
            SocketAddr::from(([10, 0, 0, 9], PORT)),
            5,
            true,
        ));
        assert!(m.an.iter().any(|a| a.rtype == T_A && a.canon == vec![10, 0, 0, 5]));
        assert!(!m.an.iter().any(|a| a.rtype == T_AAAA), "the services have no IPv6");
        assert!(!m.an.iter().any(|a| a.canon == vec![192, 168, 1, 20]), "only the address of the interface asked on");
        // all interfaces gone: waiting for one
        r.set_ifaces(t + 3.0, Vec::new());
        assert_eq!(r.state(), State::NoNetwork);
    }

    // net.ipv6 = on: AAAA for every IPv6 address of the interface asked on, no NSEC for it, a SRV brings both families
    #[test]
    fn aaaa_when_the_services_listen_on_ipv6() {
        let mut r = Responder::new(7);
        r.set_config(0.0, Config { v6: true, ..cfg() });
        let mut e = eth([192, 168, 1, 20]);
        let (ll, global): (Ipv6Addr, Ipv6Addr) =
            ("fe80::0:ff:fe00:1".parse().unwrap(), "2001:db8:5:7:1d2e:3f4a:5b6c:7d8e".parse().unwrap());
        e.v6 = vec![ll, global];
        r.set_ifaces(0.0, vec![e]);
        let mut t = 0.0;
        while r.phase != Phase::Run {
            t += 0.05;
            r.tick(t);
        }
        let from6 = SocketAddr::from((Ipv6Addr::from(0xfe80_0000_0000_0000_0000_0000_0000_0099u128), PORT));
        let m = one(&r.receive(t + 1.0, &query(0, &[(&["dho924s-204527", "local"], T_AAAA, false)]), from6, ETH, true));
        let got: Vec<Vec<u8>> = m.an.iter().filter(|a| a.rtype == T_AAAA).map(|a| a.canon.clone()).collect();
        assert_eq!(got, vec![ll.octets().to_vec(), global.octets().to_vec()]);
        assert!(m.an.iter().all(|a| a.flush) && !m.ar.iter().any(|r| r.rtype == T_NSEC));
        assert!(has(&m.ar, "dho924s-204527.local", T_A), "the other family as additional");
        let m =
            one(&r.receive(t + 2.0, &query(0, &[(&["_http", "_tcp", "local"], T_PTR, false)]), peer(PORT), ETH, true));
        assert!(has(&m.ar, "dho924s-204527.local", T_A) && has(&m.ar, "dho924s-204527.local", T_AAAA));
    }

    #[test]
    fn a_long_announcement_is_split() {
        let mut r = Responder::new(1);
        let mut c = cfg();
        for i in 0..20 {
            c.services.push(Service {
                kind: format!("_x{}._tcp", i),
                port: 1000 + i,
                txt: vec!["k=".to_string() + &"v".repeat(200)],
            });
        }
        r.set_config(0.0, c);
        r.set_ifaces(0.0, vec![eth([192, 168, 1, 20])]);
        let mut pkts = Vec::new();
        let mut t = 0.0;
        while r.phase != Phase::Run {
            t += 0.05;
            pkts.extend(r.tick(t).into_iter().filter(|o| parse(&o.pkt).unwrap().is_response()));
        }
        assert!(pkts.len() > 2, "{}", pkts.len());
        let total: usize = pkts.iter().map(|o| parse(&o.pkt).unwrap().an.len()).sum();
        assert_eq!(total, 2 * r.records(&eth([192, 168, 1, 20])).len());
        assert!(pkts.iter().all(|o| o.pkt.len() <= MAX_PKT + 300));
    }

    /// On a host: the responder on the interfaces in OOS_MDNS_IF (default lo0, or lo on Linux) with a test name, asked
    /// like `dig @224.0.0.251 -p 5353` would. With OOS_MDNS_HOLD=<s> it keeps running for `dns-sd -B _http._tcp` or
    /// `dns-sd -G v4v6 <name>.local` by hand. (On macOS a legacy query from the same host only comes back on lo0: on
    /// en0 the kernel does not loop it to the socket, so the answer check needs lo0 there; mDNSResponder's own
    /// queries and the announcements do arrive on en0. Verified 2026-11-13 with dns-sd -G and -B.)
    #[test]
    #[ignore = "live multicast: set OOS_MDNS_IF"]
    fn host_responder() {
        let only: Vec<String> =
            std::env::var("OOS_MDNS_IF").expect("set OOS_MDNS_IF").split(',').map(str::to_string).collect();
        let ifs = crate::netio::mdns::interfaces(Some(&only));
        assert!(!ifs.is_empty(), "no interface {:?}", only);
        let name = format!("oos-test-{}", std::process::id());
        let mut m = Mdns::new(crate::netio::Runtime::new().unwrap(), Some(only));
        let mut c = cfg();
        c.name = name.clone();
        c.instance = format!("OOS test ({})", std::process::id());
        m.set(c);
        crate::ui::lifecycle::test_wait::until("mDNS announcement", || {
            let current = status();
            (current.state == State::Running && current.name == name).then_some(())
        });
        assert_eq!(status().state, State::Running, "{:?}", status());
        let s = std::net::UdpSocket::bind("0.0.0.0:0").unwrap();
        let (a, _) = ifs[0].v4.expect("an IPv4 address");
        crate::netio::mdns::socket::multicast_interface(&s, a);
        s.set_read_timeout(Some(std::time::Duration::from_secs(2))).unwrap();
        s.send_to(&query(0x1234, &[(&[&name, "local"], T_A, false)]), (GROUP4, PORT)).unwrap();
        let mut buf = [0u8; 1500];
        match s.recv_from(&mut buf) {
            Ok((n, from)) => {
                let ans = parse(&buf[..n]).unwrap();
                println!("answer from {}: {:?}", from, ans);
                assert_eq!(ans.id, 0x1234);
                assert!(ans.an.iter().any(|r| r.rtype == T_A && r.canon == a.octets().to_vec()));
            }
            Err(e) if a.is_loopback() => panic!("no answer: {}", e),
            Err(e) => println!("no legacy answer on {} ({}): see above, check with dns-sd", ifs[0].name, e),
        }
        if let Some(hold) = std::env::var("OOS_MDNS_HOLD").ok().and_then(|s| s.parse::<u64>().ok()) {
            println!("{}.local for {} s", name, hold);
            std::thread::sleep(std::time::Duration::from_secs(hold));
        }
        m.shutdown(std::time::Instant::now() + std::time::Duration::from_secs(1));
        assert_eq!(status().state, State::Off);
    }
}
