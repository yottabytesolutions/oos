// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! HiSLIP (IVI-6.1) on TCP 4880; the scope enables it with `remote.hislip` (off by default).
//!
//! A connection is accepted (from the addresses of `net.allow`, `netacl`) and the data payload is handed to the same
//! SCPI interpreter as the raw socket.
//! The session itself is not part of this: no lock is kept, overlap mode is not run, and remote/local is only
//! acknowledged. What is here is the header, the initial exchange that lets a client send that payload, and the
//! reply that carries the SCPI response back.

#[cfg(test)]
pub use crate::netport::hislip::take_message;
pub use crate::netport::hislip::{encode, Message, PORT};
use crate::netport::{hislip::*, Output};
#[derive(Clone, Copy, PartialEq)]
enum Role {
    New,
    Sync,
    Async,
}

struct Peer {
    role: Role,
    session: u16,
    pending: Vec<u8>,
    msgid: u32,
    program: Option<crate::scpi::Program>,
}

struct Live {
    id: u16,
    /// Largest SCPI response payload we will put in one message toward this client.
    max_payload: usize,
}

#[derive(Default)]
struct Out {
    output: Output,
    close: bool,
}

fn answer(m: Vec<u8>) -> Out {
    Out { output: Output::bytes(m), close: false }
}
fn quiet() -> Out {
    Out::default()
}
fn fatal(code: u8) -> Out {
    Out { close: true, ..answer(encode(FATAL_ERROR, code, 0, b"")) }
}
fn error(code: u8) -> Out {
    answer(encode(ERROR, code, 0, b""))
}

fn pump(
    peer: &mut Peer,
    live: &[Live],
    exec: &mut dyn FnMut(&mut crate::scpi::Program) -> Option<Vec<u8>>,
) -> Option<Out> {
    let Some(mut program) = peer.program.take() else {
        return Some(quiet());
    };
    let Some(mut response) = exec(&mut program) else {
        peer.program = Some(program);
        return None;
    };
    if !program.has_reply {
        return Some(quiet());
    }
    response.reserve_exact(1);
    response.push(b'\n');
    let max = live.iter().find(|l| l.id == peer.session).map_or(MAX_PAYLOAD, |l| l.max_payload);
    Some(Out { output: Output::hislip(response, peer.msgid, max), close: false })
}

fn subaddress_ok(data: &[u8]) -> bool {
    let s = data.split(|&b| b == 0).next().unwrap_or(data);
    s.is_empty() || s == b"hislip0"
}

fn alloc_id(live: &[Live], next: &mut u16) -> u16 {
    loop {
        let id = *next;
        *next = next.wrapping_add(1);
        if id != 0 && live.iter().all(|l| l.id != id) {
            return id;
        }
    }
}

fn on_program_message(
    peer: &mut Peer,
    live: &mut Vec<Live>,
    next: &mut u16,
    msg: &Message,
    exec: &mut dyn FnMut(&mut crate::scpi::Program) -> Option<Vec<u8>>,
) -> Out {
    match msg.typ {
        INITIALIZE if peer.role == Role::New => {
            let client = (msg.param >> 16) as u16;
            // 1.0 is the oldest version we speak; a newer client is answered at 1.0
            if client < VERSION || !subaddress_ok(&msg.data) {
                return fatal(FATAL_BAD_INIT);
            }
            let id = alloc_id(live, next);
            live.push(Live { id, max_payload: MAX_PAYLOAD });
            peer.role = Role::Sync;
            peer.session = id;
            answer(encode(INITIALIZE_RESPONSE, 0, (VERSION as u32) << 16 | id as u32, b""))
        }
        ASYNC_INITIALIZE if peer.role == Role::New => {
            let id = msg.param as u16;
            if !live.iter().any(|l| l.id == id) {
                return fatal(FATAL_BAD_INIT);
            }
            peer.role = Role::Async;
            peer.session = id;
            answer(encode(ASYNC_INITIALIZE_RESPONSE, 0, 0, b""))
        }
        DATA | DATA_END if peer.role == Role::Sync => {
            if peer.pending.len() + msg.data.len() > MAX_PAYLOAD {
                peer.pending.clear();
                return error(ERR_LARGE);
            }
            peer.pending.extend_from_slice(&msg.data);
            peer.msgid = msg.param;
            if msg.typ == DATA {
                return quiet();
            }
            let payload = std::mem::take(&mut peer.pending);
            peer.program = Some(crate::scpi::Program::new(&String::from_utf8_lossy(&payload)));
            pump(peer, live, exec).unwrap_or_default()
        }
        ASYNC_MAXIMUM_MESSAGE_SIZE if peer.role != Role::New => {
            let n = msg.data.get(..8).and_then(|b| b.try_into().ok()).map(u64::from_be_bytes).unwrap_or(0);
            // the size covers the header; an empty value keeps the default, a small one gives MIN_PAYLOAD
            let payload = match n.checked_sub(HEADER as u64) {
                Some(p) if p > 0 => (p.min(MAX_PAYLOAD as u64) as usize).max(MIN_PAYLOAD),
                _ => MAX_PAYLOAD,
            };
            if let Some(l) = live.iter_mut().find(|l| l.id == peer.session) {
                l.max_payload = payload;
            }
            let ours = MAX_PAYLOAD as u64 + HEADER as u64;
            answer(encode(ASYNC_MAXIMUM_MESSAGE_SIZE_RESPONSE, 0, 0, &ours.to_be_bytes()))
        }
        ASYNC_LOCK if peer.role != Role::New => {
            // acknowledged so the payload can follow; nothing is stored
            answer(encode(ASYNC_LOCK_RESPONSE, 1, 0, b""))
        }
        ASYNC_LOCK_INFO if peer.role != Role::New => answer(encode(ASYNC_LOCK_INFO_RESPONSE, 0, 0, b"")),
        ASYNC_STATUS_QUERY if peer.role != Role::New => answer(encode(ASYNC_STATUS_RESPONSE, 0, 0, b"")),
        ASYNC_REMOTE_LOCAL_CONTROL if peer.role != Role::New => answer(encode(ASYNC_REMOTE_LOCAL_RESPONSE, 0, 0, b"")),
        DEVICE_CLEAR_COMPLETE if peer.role == Role::Sync => {
            peer.pending.clear();
            peer.program = None;
            answer(encode(DEVICE_CLEAR_ACKNOWLEDGE, 0, 0, b""))
        }
        ASYNC_DEVICE_CLEAR if peer.role == Role::Async => answer(encode(ASYNC_DEVICE_CLEAR_ACKNOWLEDGE, 0, 0, b"")),
        // a trigger carries no SCPI payload
        12 if peer.role == Role::Sync => quiet(),
        _ if peer.role == Role::New => fatal(FATAL_BAD_INIT),
        _ => error(ERR_TYPE),
    }
}

#[cfg(test)]
fn on_message(
    peer: &mut Peer,
    live: &mut Vec<Live>,
    next: &mut u16,
    msg: &Message,
    exec: &mut dyn FnMut(&str) -> Vec<u8>,
) -> Out {
    on_program_message(peer, live, next, msg, &mut |program| Some(program.immediate(exec)))
}

mod ports;
pub use ports::Server;

#[cfg(test)]
mod tests;
