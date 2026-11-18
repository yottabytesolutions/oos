// SPDX-License-Identifier: GPL-2.0-or-later
//! Frame-owned sessions and continuations behind the shared network port.
use super::*;
use crate::netport::{Envelope, Ports, RemoteRequest, Reservation, Wake};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;

type Active = (Envelope, Reservation);
struct Conn {
    id: u64,
    closed: Arc<AtomicBool>,
    cleared: Arc<AtomicBool>,
    peer: Peer,
    active: Option<Active>,
}

pub struct Server {
    endpoint: crate::netio::Endpoint,
    conns: Vec<Conn>,
    live: Vec<Live>,
    next: u16,
    pub acl: crate::netacl::Shared,
    parked: Option<crate::netport::Reply>,
}
impl Server {
    pub fn closed() -> Self {
        Self {
            endpoint: Default::default(),
            conns: Vec::new(),
            live: Vec::new(),
            next: 1,
            acl: Default::default(),
            parked: None,
        }
    }
    pub fn open_with_runtime(
        port: impl Into<crate::net6::Port>,
        runtime: crate::netio::Runtime,
        wake: Arc<Wake>,
    ) -> Self {
        let mut server = Self::closed();
        server.endpoint = crate::netio::Endpoint::new(Some(runtime), wake);
        server.endpoint.set_open(Some(port.into()), false, crate::netio::Protocol::Hislip);
        server
    }
    #[cfg(test)]
    pub fn open(port: impl Into<crate::net6::Port>) -> Self {
        Self::open_with_runtime(port, crate::netio::Runtime::new().unwrap(), Arc::new(Wake::new().unwrap()))
    }
    pub fn port(&self) -> Option<u16> {
        self.endpoint.port()
    }
    #[cfg(test)]
    pub fn poll(&mut self, exec: &mut dyn FnMut(&str) -> Vec<u8>) {
        self.poll_async(&mut |program| Some(program.immediate(exec)));
    }
    pub fn poll_async(&mut self, exec: &mut dyn FnMut(&mut crate::scpi::Program) -> Option<Vec<u8>>) {
        let Some(ports) = self.endpoint.ports(&self.acl) else { return };
        let start = std::time::Instant::now();
        self.poll_ports(&ports, || start.elapsed(), exec);
    }
    fn poll_ports(
        &mut self,
        ports: &Ports,
        mut elapsed: impl FnMut() -> Duration,
        exec: &mut dyn FnMut(&mut crate::scpi::Program) -> Option<Vec<u8>>,
    ) {
        self.conns.retain_mut(|conn| {
            if conn.closed.load(Ordering::Acquire) {
                if conn.peer.role == Role::Sync {
                    self.live.retain(|l| l.id != conn.peer.session);
                }
                return false;
            }
            if conn.active.as_ref().is_some_and(|(request, _)| request.cancelled()) {
                conn.active = None;
                conn.peer.program = None;
            }
            true
        });
        let mut remaining = 8;
        while remaining != 0 {
            if elapsed() >= Duration::from_micros(500) {
                ports.notify_frame();
                break;
            }
            if !ports.flush(&mut self.parked) {
                return;
            }
            let Some(reservation) = ports.budget().reserve(REPLY_BYTES) else { break };
            let Some(request) = ports.requests().take(1).pop() else { break };
            if request.cancelled() {
                continue;
            }
            remaining -= 1;
            self.dispatch(request, reservation, exec);
        }
        for conn in &mut self.conns {
            if self.parked.is_some() || remaining == 0 || elapsed() >= Duration::from_micros(500) {
                break;
            }
            let Some((request, reservation)) = conn.active.take() else { continue };
            remaining -= 1;
            let Some(out) = pump(&mut conn.peer, &self.live, exec) else {
                conn.active = Some((request, reservation));
                continue;
            };
            self.parked = Some(reply(request, reservation, out));
        }
        if self.parked.is_some() {
            ports.notify_frame();
        }
    }
    fn dispatch(
        &mut self,
        request: Envelope,
        reservation: Reservation,
        exec: &mut dyn FnMut(&mut crate::scpi::Program) -> Option<Vec<u8>>,
    ) {
        let RemoteRequest::Hislip { message, closed, cleared } = request.request() else { return };
        let id = request.id().connection();
        let index = self.conns.iter().position(|c| c.id == id).unwrap_or_else(|| {
            self.conns.push(Conn {
                id,
                closed: closed.clone(),
                cleared: cleared.clone(),
                peer: Peer { role: Role::New, session: 0, pending: Vec::new(), msgid: 0, program: None },
                active: None,
            });
            self.conns.len() - 1
        });
        let out = message.as_ref().map_or_else(
            || fatal(FATAL_BAD_HEADER),
            |msg| on_program_message(&mut self.conns[index].peer, &mut self.live, &mut self.next, msg, exec),
        );
        if message.as_ref().is_some_and(|m| matches!(m.typ, DEVICE_CLEAR_COMPLETE | ASYNC_DEVICE_CLEAR)) {
            let session = self.conns[index].peer.session;
            for conn in self.conns.iter_mut().filter(|c| c.peer.session == session) {
                conn.cleared.store(true, Ordering::Release);
                conn.peer.pending.clear();
                conn.peer.program = None;
            }
        }
        if self.conns[index].peer.program.is_some() {
            self.conns[index].active = Some((request, reservation));
        } else {
            self.parked = Some(reply(request, reservation, out));
        }
    }
}

fn reply(request: Envelope, reservation: Reservation, out: Out) -> crate::netport::Reply {
    let mut reply = crate::netport::Reply::from_output(request.id(), out.output, reservation);
    reply.set_close(out.close);
    reply
}

#[cfg(test)]
mod tests;
