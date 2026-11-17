// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! 1-Wire, on the bus line and through a DS2482 I2C bridge.

use super::{ow_crc8, Kind, Packet};
use crate::edges::edges;

// 1-Wire

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum OwPhase {
    /// before the first reset: bytes are shown as Data
    Idle,
    /// the ROM command follows
    Rom,
    /// 8 ROM code bytes (READ / MATCH ROM)
    Code,
    /// 64 search triplets (SEARCH ROM / ALARM SEARCH)
    Search,
    /// function command and data until the next reset
    Func,
}

/// 1-Wire network layer above the bit level (shared by `onewire` and `ds2482`).
struct OwNet {
    out: Vec<Packet>,
    phase: OwPhase,
    /// bits of the byte or search triplet being assembled
    bits: Vec<(bool, f64, f64)>,
    /// search: chosen directions = ROM code bits
    dirs: Vec<(bool, f64, f64)>,
    code: Vec<(u8, f64, f64)>,
    /// set by OVERDRIVE SKIP / MATCH ROM: the following time slots run at overdrive speed
    overdrive: bool,
}

impl OwNet {
    fn new(overdrive: bool) -> OwNet {
        OwNet { out: Vec::new(), phase: OwPhase::Idle, bits: Vec::new(), dirs: Vec::new(), code: Vec::new(), overdrive }
    }

    fn reset(&mut self, t0: f64, t1: f64) {
        self.bits.clear();
        self.dirs.clear();
        self.code.clear();
        self.phase = OwPhase::Rom;
        self.out.push(Packet::tag("!RESET", t0, t1, true));
    }

    fn presence(&mut self, t0: f64, t1: f64, ok: bool) {
        self.out.push(Packet::tag("!PRESENCE", t0, t1, ok));
    }

    fn bit(&mut self, b: bool, t0: f64, t1: f64) {
        self.bits.push((b, t0, t1));
        if self.phase == OwPhase::Search {
            if self.bits.len() < 3 {
                return;
            }
            let (id, cmp, dir) = (self.bits[0].0, self.bits[1].0, self.bits[2].0);
            let ts = self.bits[0].1;
            self.bits.clear();
            if id && cmp {
                // no device answered: the search ends
                self.out.push(Packet::tag("!ERR", ts, t1, false));
                self.phase = OwPhase::Func;
                return;
            }
            self.dirs.push((dir, ts, t1));
            if self.dirs.len() == 8 {
                let v = self.dirs.iter().enumerate().fold(0u8, |a, (i, d)| a | (d.0 as u8) << i);
                let (s, e) = (self.dirs[0].1, self.dirs[7].2);
                self.dirs.clear();
                self.code.push((v, s, e));
                if self.code.len() == 8 {
                    self.rom_code();
                    self.phase = OwPhase::Func;
                }
            }
            return;
        }
        if self.bits.len() == 8 {
            let v = self.bits.iter().enumerate().fold(0u8, |a, (i, d)| a | (d.0 as u8) << i);
            let s = self.bits[0].1;
            self.byte(v, s, t1);
        }
    }

    fn byte(&mut self, v: u8, t0: f64, t1: f64) {
        self.bits.clear();
        match self.phase {
            OwPhase::Rom => {
                self.out.push(Packet::field("ROM", t0, t1, v as u32, true));
                self.phase = match v {
                    0x33 | 0x0f | 0x55 | 0x69 => OwPhase::Code,
                    0xf0 | 0xec => OwPhase::Search,
                    _ => OwPhase::Func,
                };
                if v == 0x3c || v == 0x69 {
                    self.overdrive = true;
                }
            }
            OwPhase::Code => {
                self.code.push((v, t0, t1));
                if self.code.len() == 8 {
                    self.rom_code();
                    self.phase = OwPhase::Func;
                }
            }
            _ => self.out.push(Packet::new(Kind::Data, t0, t1, v as u32, true)),
        }
    }

    /// The 64-bit ROM code: family code, 48-bit serial number, CRC-8.
    fn rom_code(&mut self) {
        let c = std::mem::take(&mut self.code);
        let b = |i: usize| c[i].0 as u32;
        self.out.push(Packet::field("FAM", c[0].1, c[0].2, b(0), true));
        self.out.push(
            Packet::field("SER", c[1].1, c[6].2, b(1) | b(2) << 8 | b(3) << 16 | b(4) << 24, true)
                .with_value2(b(5) | b(6) << 8),
        );
        let bytes: Vec<u8> = c.iter().map(|x| x.0).collect();
        self.out.push(Packet::new(Kind::Crc, c[7].1, c[7].2, b(7), ow_crc8(&bytes[..7]) == bytes[7]));
    }
}

/// 1-Wire (timings of Maxim/Analog Devices AN126, standard and overdrive speed) on the bus line, idle high. Low
/// pulses are classified by length: standard: < 15 us = 1, < 150 us = 0, >= 300 us = reset; overdrive: < 2 us = 1,
/// < 40 us = 0, >= 40 us = reset; a reset >= 400 us returns to standard speed, OVERDRIVE SKIP/MATCH ROM (0x3C/0x69)
/// switch to overdrive. `overdrive` = speed at the record start.
///
/// Emits Field("!RESET"), Field("!PRESENCE") (`ok` = a presence pulse followed), Field("ROM") = the ROM command;
/// READ/MATCH ROM and SEARCH ROM / ALARM SEARCH (the direction bits of the 64 triplets) give the ROM code as
/// Field("FAM") (family), Field("SER") (48-bit serial: value = low 32 bits, value2 = high 16) and Crc (`ok` = CRC-8
/// matches); a search where no device answers gives Field("!ERR"). All other bytes (function commands, data) are
/// Data, LSB first. Pulses of no valid length: Field("!ERR").
pub fn onewire(v: &[f32], dt: f64, thr: f32, hyst: f32, overdrive: bool) -> Vec<Packet> {
    let us = 1e-6 / dt;
    let mut net = OwNet::new(overdrive);
    if us.is_nan() || us < 0.5 {
        return net.out;
    }
    let e = edges(v, thr, hyst);
    let p: Vec<(f64, f64)> = e.windows(2).filter(|w| !w[0].rising && w[1].rising).map(|w| (w[0].t, w[1].t)).collect();
    let mut i = 0;
    while i < p.len() {
        let (f, r) = p[i];
        i += 1;
        let w = (r - f) / us;
        let (one, zero, rst, slot) = if net.overdrive { (2.0, 40.0, 40.0, 8.0) } else { (15.0, 150.0, 300.0, 60.0) };
        if w >= rst {
            if w >= 400.0 {
                net.overdrive = false;
            }
            net.reset(f, r);
            // presence: the slaves pull low 15..60 us after the reset (overdrive 2..6) for 60..240 us (8..24)
            let (wait, plo, phi, win) = if net.overdrive { (10.0, 4.0, 40.0, 10.0) } else { (80.0, 30.0, 300.0, 70.0) };
            match p.get(i) {
                Some(&(pf, pr)) if pf - r <= wait * us && (pr - pf) / us >= plo && (pr - pf) / us < phi => {
                    net.presence(pf, pr, true);
                    i += 1;
                }
                _ => net.presence(r, r + win * us, false),
            }
        } else if w < zero {
            net.bit(w < one, f, r.max(f + slot * us));
        } else {
            net.out.push(Packet::tag("!ERR", f, r, false));
        }
    }
    net.out
}

/// DS2482 register pointer codes.
const DS_STATUS: u8 = 0xf0;

const DS_DATA: u8 = 0xe1;

#[derive(Clone, Copy, Debug)]
enum DsPend {
    Reset(f64),
    Bit(bool, f64),
    Read(f64),
    Triplet(bool, f64),
}

type DsTransfer = (bool, f64, Vec<(u8, f64, f64)>); // (read, t0, bytes)

/// The I2C transfers to a DS2482 (device addresses 0x18..0x1F) in `decode::i2c` packets.
fn ds2482_transfers(i2c: &[Packet]) -> Vec<DsTransfer> {
    let mut xfers: Vec<DsTransfer> = Vec::new();
    let mut cur: Option<DsTransfer> = None;
    for p in i2c {
        match p.kind {
            Kind::Addr { read } => {
                xfers.extend(cur.take());
                if (0x18..=0x1f).contains(&p.value) {
                    cur = Some((read, p.t0, Vec::new()));
                }
            }
            Kind::Data => {
                if let Some(c) = cur.as_mut() {
                    c.2.push((p.value as u8, p.t0, p.t1));
                }
            }
            Kind::Start | Kind::RepStart | Kind::Stop => xfers.extend(cur.take()),
            _ => {}
        }
    }
    xfers.extend(cur.take());
    xfers
}

/// The DS2482 command interpreter above the 1-Wire network layer.
struct Ds2482 {
    net: OwNet,
    /// the register the read pointer selects
    ptr: u8,
    /// the 1-Wire command waiting for its result
    pend: Option<DsPend>,
    /// start of a 1-Wire read byte whose value the next read of the read data register gives
    read_ready: Option<f64>,
}

impl Ds2482 {
    /// Completes the pending 1-Wire command; `status` = the status byte that showed 1WB = 0, if polled.
    fn finish(&mut self, status: Option<u8>, t1: f64) {
        let Some(pend) = self.pend.take() else { return };
        let net = &mut self.net;
        match pend {
            DsPend::Reset(t0) => {
                net.reset(t0, t1);
                if let Some(s) = status {
                    net.presence(t0, t1, s & 0x02 != 0 && s & 0x04 == 0);
                }
            }
            DsPend::Bit(b, t0) => net.bit(status.map_or(b, |s| s & 0x20 != 0), t0, t1),
            DsPend::Triplet(dir, t0) => {
                let (id, cmp, d) = status.map_or((dir, !dir, dir), |s| (s & 0x20 != 0, s & 0x40 != 0, s & 0x80 != 0));
                net.bit(id, t0, t1);
                net.bit(cmp, t0, t1);
                net.bit(d, t0, t1);
            }
            DsPend::Read(t0) => self.read_ready = Some(t0),
        }
    }

    /// One byte read from the register the read pointer selects.
    fn read(&mut self, v: u8, t1: f64) {
        match self.ptr {
            DS_STATUS if v & 0x01 == 0 => self.finish(Some(v), t1),
            DS_DATA => {
                let start = match self.pend {
                    Some(DsPend::Read(s)) => {
                        self.pend = None;
                        Some(s)
                    }
                    _ => self.read_ready.take(),
                };
                if let Some(s) = start {
                    self.net.byte(v, s, t1);
                }
            }
            _ => {}
        }
    }

    /// A 1-Wire command: it runs until a status read shows it done.
    fn start(&mut self, pend: DsPend) {
        self.ptr = DS_STATUS;
        self.pend = Some(pend);
    }

    /// A command byte with its parameter byte.
    fn command(&mut self, cmd: u8, par: Option<u8>, t0: f64, t1: f64) {
        // a new 1-Wire or device command completes the previous one (not Set Read Pointer, used for polling)
        if cmd != 0xe1 {
            self.finish(None, t0);
        }
        match (cmd, par) {
            (0xf0, _) => {
                self.ptr = DS_STATUS;
                self.net.out.push(Packet::tag("!DRST", t0, t1, true));
            }
            (0xe1, Some(p)) => self.ptr = p,
            (0xd2, Some(c)) => {
                self.ptr = 0xc3;
                self.net.out.push(Packet::field("CFG", t0, t1, (c & 0x0f) as u32, c >> 4 == !c & 0x0f));
            }
            (0xc3, Some(c)) => {
                self.ptr = 0xd2;
                let ch = [0xf0, 0xe1, 0xd2, 0xc3, 0xb4, 0xa5, 0x96, 0x87].iter().position(|&x| x == c);
                self.net.out.push(Packet::field("CH", t0, t1, ch.unwrap_or(0) as u32, ch.is_some()));
            }
            (0xb4, _) => self.start(DsPend::Reset(t0)),
            (0x87, Some(x)) => self.start(DsPend::Bit(x & 0x80 != 0, t0)),
            (0xa5, Some(x)) => {
                self.ptr = DS_STATUS;
                self.net.byte(x, t0, t1);
            }
            (0x96, _) => self.start(DsPend::Read(t0)),
            (0x78, Some(x)) => self.start(DsPend::Triplet(x & 0x80 != 0, t0)),
            _ => {}
        }
    }
}

/// 1-Wire through a DS2482-100/-800 I2C bridge: interprets the command stream (packets from `decode::i2c`, device
/// addresses 0x18..0x1F) as 1-Wire activity and decodes it like `onewire` (Field "!RESET", "!PRESENCE", "ROM", "FAM",
/// "SER", Crc, Data). Device reset gives Field("!DRST"), Write Configuration Field("CFG") (value = config nibble,
/// `ok` = the complement nibble is right), Channel Select Field("CH") (value = channel 0..7). Results of 1-Wire
/// reset / single bit / triplet come from the next status read with 1WB = 0 (presence = PPD and not SD); a read
/// byte from the following read of the read data register (pointer 0xE1). Unpolled commands complete at the next command.
pub fn ds2482(i2c: &[Packet]) -> Vec<Packet> {
    let mut ds = Ds2482 { net: OwNet::new(false), ptr: DS_STATUS, pend: None, read_ready: None };
    for (read, t0, b) in ds2482_transfers(i2c) {
        if read {
            b.iter().for_each(|&(v, _, t1)| ds.read(v, t1));
        } else if let Some(&(cmd, _, _)) = b.first() {
            let t1 = b.last().map_or(t0, |x| x.2);
            ds.command(cmd, b.get(1).map(|x| x.0), t0, t1);
        }
    }
    let t = ds.net.out.last().map_or(0.0, |p| p.t1);
    ds.finish(None, t);
    ds.net.out
}
