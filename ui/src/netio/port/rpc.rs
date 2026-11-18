// SPDX-License-Identifier: GPL-2.0-or-later
//! VXI-11 RPC wire values shared by IO framing and frame-owned link handling.
pub const PMAP_PROG: u32 = 100_000;
pub const CORE_PROG: u32 = 0x0607AF;
pub const CORE_PORT: u16 = 9009;
// A link can append one RESP_MAX program to an output just below RESP_MAX before its next pump.
// Keep that existing payload bound, with room for XDR and record/chunk headers.
pub(crate) const REPLY_BYTES: usize = 2 * super::RESPONSE_BYTES + 4096;
/// Most data waiting on one connection: maxRecvSize of create_link and room for the RPC header.
pub(crate) const MAX_RECORD: usize = (1 << 20) + 4096;

#[derive(Clone, Copy)]
pub enum Channel {
    Core,
    Mapper { port: u16, datagram: bool },
}

/// XDR reader over a call body.
pub struct Xdr<'a> {
    b: &'a [u8],
    p: usize,
}

impl<'a> Xdr<'a> {
    pub fn new(b: &'a [u8]) -> Xdr<'a> {
        Xdr { b, p: 0 }
    }
    pub fn u32(&mut self) -> Option<u32> {
        let v = self.b.get(self.p..self.p + 4)?;
        self.p += 4;
        Some(u32::from_be_bytes([v[0], v[1], v[2], v[3]]))
    }
    pub fn opaque(&mut self) -> Option<&'a [u8]> {
        let n = self.u32()? as usize;
        let v = self.b.get(self.p..self.p + n)?;
        self.p += n.div_ceil(4) * 4;
        Some(v)
    }
}

pub(crate) fn put(out: &mut Vec<u8>, v: u32) {
    words(out, &[v]);
}

pub(crate) fn words(out: &mut Vec<u8>, values: &[u32]) {
    out.extend(values.iter().flat_map(|v| v.to_be_bytes()));
}

pub(crate) fn put_opaque(out: &mut Vec<u8>, d: &[u8]) {
    out.reserve_exact(4 + d.len().div_ceil(4) * 4);
    put(out, d.len() as u32);
    out.extend_from_slice(d);
    out.resize(out.len() + (4 - d.len() % 4) % 4, 0);
}

/// A parsed RPC call: xid, program, version, procedure and the argument bytes.
pub struct Call<'a> {
    pub xid: u32,
    pub prog: u32,
    pub vers: u32,
    pub proc_: u32,
    pub args: &'a [u8],
}

pub fn parse_call(m: &[u8]) -> Option<Call<'_>> {
    let mut x = Xdr::new(m);
    let xid = x.u32()?;
    if x.u32()? != 0 || x.u32()? != 2 {
        return None; // not a call / not RPC v2
    }
    let (prog, vers, proc_) = (x.u32()?, x.u32()?, x.u32()?);
    x.u32()?;
    x.opaque()?; // credentials
    x.u32()?;
    x.opaque()?; // verifier
                 // a verifier without its padding leaves the position past the end
    Some(Call { xid, prog, vers, proc_, args: m.get(x.p..)? })
}

/// Accepted reply header (verifier AUTH_NONE) with `accept_stat` (0 success, 1 prog unavailable, 3 proc unavailable).
pub(crate) fn reply(xid: u32, accept: u32) -> Vec<u8> {
    let mut o = Vec::with_capacity(64);
    words(&mut o, &[xid, 1, 0, 0, 0, accept]);
    o
}

/// Portmapper: GETPORT (3) of the core channel over TCP (6) -> `CORE_PORT`; NULL (0); anything else 0.
pub fn portmap(c: &Call, core_port: u16) -> Vec<u8> {
    if c.prog != PMAP_PROG {
        return reply(c.xid, 1);
    }
    let mut o = reply(c.xid, 0);
    match c.proc_ {
        0 => {}
        3 => {
            let mut x = Xdr::new(c.args);
            let (prog, _vers, prot) = (x.u32().unwrap_or(0), x.u32().unwrap_or(0), x.u32().unwrap_or(0));
            put(&mut o, if prog == CORE_PROG && prot == 6 { core_port as u32 } else { 0 });
        }
        _ => return reply(c.xid, 3),
    }
    o
}

/// Removes one complete RPC record (all fragments up to the last) from `buf`.
pub fn take_record(buf: &mut Vec<u8>) -> Option<Vec<u8>> {
    let mut p = 0;
    let mut msg = Vec::new();
    loop {
        let h = buf.get(p..p + 4)?;
        let h = u32::from_be_bytes([h[0], h[1], h[2], h[3]]);
        let n = (h & 0x7fff_ffff) as usize;
        let frag = buf.get(p + 4..p + 4 + n)?;
        msg.extend_from_slice(frag);
        p += 4 + n;
        if h & 1 << 31 != 0 {
            buf.drain(..p);
            return Some(msg);
        }
    }
}
