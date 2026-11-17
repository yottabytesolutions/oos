// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later
//! Bounded HTTP input with explicit time sampling for deadline tests.
use super::*;

/// One connection: the head (`MAX_HEAD`), the early refusals, `100 Continue` when asked for, the body within the
/// limits, then the request to the frame loop; 503 when the frame loop does not take it in `Limits::take`.
pub(super) fn reader(s: TcpStream, inbox: &Inbox, acl: &Mutex<crate::netacl::Shared>, lim: Limits) {
    reader_with_now(s, inbox, acl, lim, Instant::now);
}

pub(super) fn reader_with_now(
    s: TcpStream,
    inbox: &Inbox,
    acl: &Mutex<crate::netacl::Shared>,
    lim: Limits,
    now: impl Fn() -> Instant,
) {
    let input = Input { socket: &s, limit: lim, started: now(), now };
    let _ = s.set_nonblocking(false);
    let _ = s.set_nodelay(true);
    let (mut req, mut buf): (Vec<u8>, _) = (Vec::new(), [0u8; 65536]);
    let end = loop {
        if let Some(p) = req.windows(4).position(|w| w == b"\r\n\r\n") {
            break p + 4;
        }
        if req.len() > MAX_HEAD {
            return answer(&s, "431 Request Header Fields Too Large", "request head too long");
        }
        if input.read(&mut req, &mut buf).is_err() {
            return;
        }
    };
    let text = String::from_utf8_lossy(&req[..end]).into_owned();
    let h = parse_head(&text);
    let peer_ok = acl.lock().unwrap_or_else(|e| e.into_inner()).clone().permits_peer(s.peer_addr());
    if let Some((status, msg)) = refuse_early(&h, &text, peer_ok) {
        return answer(&s, status, msg);
    }
    let Some(_slot) = limits::LargeSlot::take(&h, &inbox.large) else {
        return answer(&s, "503 Service Unavailable", "busy, try again");
    };
    let need = end + h.len.unwrap_or(0);
    req.reserve_exact(need.saturating_sub(req.len())); // one allocation, not a doubling past it
    if h.expect_continue && req.len() < need && !limits::send_continue(&s) {
        return;
    }
    while req.len() < need {
        if input.read(&mut req, &mut buf).is_err() {
            return;
        }
    }
    req.truncate(need);
    let body = req.split_off(end);
    shutdown::queue_request(s, inbox, text, body, lim);
}

struct Input<'a, F> {
    socket: &'a TcpStream,
    limit: Limits,
    started: Instant,
    now: F,
}
impl<F: Fn() -> Instant> Input<'_, F> {
    fn read(&self, req: &mut Vec<u8>, buf: &mut [u8]) -> Result<(), ()> {
        use std::io::Read;
        let left = self.limit.deadline.saturating_sub((self.now)().saturating_duration_since(self.started));
        if left.is_zero() {
            answer(self.socket, "408 Request Timeout", "request took too long");
            return Err(());
        }
        let _ = self.socket.set_read_timeout(Some(self.limit.idle.min(left)));
        let mut socket = self.socket;
        match socket.read(buf) {
            Ok(0) => Err(()),
            Ok(n) => {
                req.extend_from_slice(&buf[..n]);
                Ok(())
            }
            Err(e) if crate::net::later(&e) => {
                answer(self.socket, "408 Request Timeout", "request timed out");
                Err(())
            }
            Err(_) => Err(()),
        }
    }
}
