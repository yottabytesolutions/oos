// SPDX-License-Identifier: GPL-2.0-or-later
//! Frame-owned RPC requests, link ownership, continuations and parked replies.
use super::*;
use crate::netport::{Envelope, Output, Ports, RemoteRequest, Reply, Wake};
use std::sync::{atomic::Ordering, Arc};

pub struct Server {
    endpoint: crate::netio::Endpoint,
    links: Vec<Option<Link>>,
    pub acl: crate::netacl::Shared,
    parked: Option<Reply>,
    pending: Option<Envelope>,
}
impl Server {
    pub fn closed(runtime: crate::netio::Runtime, wake: Arc<Wake>) -> Self {
        Self {
            endpoint: crate::netio::Endpoint::new(Some(runtime), wake),
            links: Vec::new(),
            acl: Default::default(),
            parked: None,
            pending: None,
        }
    }
    pub fn host(runtime: crate::netio::Runtime, wake: Arc<Wake>) -> std::io::Result<Self> {
        let mut server = Self::closed(runtime, wake);
        server.endpoint.io = Some(crate::netio::Service::start(
            crate::net6::Port::Host,
            false,
            server.endpoint.wake.clone(),
            server.endpoint.runtime()?,
            crate::netio::Protocol::Rpc(crate::netport::rpc::Channel::Core),
        )?);
        Ok(server)
    }
    pub fn set_open(&mut self, on: bool, v6: bool) {
        self.endpoint.set_open(on.then_some(CORE_PORT.into()), v6, crate::netio::Protocol::Rpc(Channel::Core));
        if !on {
            self.links.clear();
            self.parked = None;
            self.pending = None;
        }
    }
    pub fn service_port(&self) -> Option<u16> {
        self.endpoint.port()
    }
    pub fn ports(&self) -> Option<(u16, u16, u16)> {
        self.endpoint.io.as_ref()?.vxi_ports()
    }
    pub fn release_buffers(&mut self) {
        for link in self.links.iter_mut().flatten() {
            link.out = Vec::new();
        }
    }
    pub fn poll_async(&mut self, exec: &mut dyn FnMut(&mut crate::scpi::Program) -> Option<Vec<u8>>) {
        let Some(ports) = self.endpoint.ports(&self.acl) else { return };
        let start = std::time::Instant::now();
        self.poll(&ports, || start.elapsed(), exec);
    }
    fn poll(
        &mut self,
        ports: &Ports,
        mut elapsed: impl FnMut() -> std::time::Duration,
        exec: &mut dyn FnMut(&mut crate::scpi::Program) -> Option<Vec<u8>>,
    ) {
        for link in &mut self.links {
            if link.as_ref().and_then(|l| l.owner.as_ref()).is_some_and(|closed| closed.load(Ordering::Acquire)) {
                *link = None;
            }
        }
        for _ in 0..8 {
            if elapsed() >= std::time::Duration::from_micros(500) {
                ports.notify_frame();
                break;
            }
            if !ports.flush(&mut self.parked) {
                break;
            }
            let Some(request) = self.pending.take().or_else(|| ports.requests().take(1).pop()) else { break };
            if request.cancelled() {
                continue;
            }
            if !self.dispatch(request, ports, exec) {
                break;
            }
        }
        for link in self.links.iter_mut().flatten() {
            pump_link(link, exec);
        }
        if self.parked.is_some() {
            ports.notify_frame();
        }
    }
    fn dispatch(
        &mut self,
        request: Envelope,
        ports: &Ports,
        exec: &mut dyn FnMut(&mut crate::scpi::Program) -> Option<Vec<u8>>,
    ) -> bool {
        let RemoteRequest::Vxi11 { record, channel, closed } = request.request() else { return true };
        let Some(call) = parse_call(record) else {
            self.parked = ports.budget().reserve(0).map(|r| Reply::new(request.id(), Vec::new(), r));
            return true;
        };
        let required = if matches!(channel, Channel::Core) && call.proc_ == 12 {
            let mut args = Xdr::new(call.args);
            let lid = args.u32().unwrap_or(u32::MAX) as usize;
            let n = args.u32().unwrap_or(0) as usize;
            self.links.get(lid).and_then(Option::as_ref).map_or(0, |l| l.out.len().min(n.max(1)))
        } else {
            0
        };
        let Some(reservation) = ports.budget().reserve(required.div_ceil(4) * 4 + 256) else {
            self.pending = Some(request);
            return false;
        };
        let bytes = match channel {
            Channel::Core => core_async(&call, &mut self.links, Some(closed.clone()), exec),
            Channel::Mapper { port, .. } => portmap(&call, *port),
        };
        self.parked = Some(if matches!(channel, Channel::Mapper { datagram: true, .. }) {
            Reply::new(request.id(), bytes, reservation)
        } else {
            Reply::from_output(request.id(), Output::rpc(bytes), reservation)
        });
        true
    }
}

#[cfg(test)]
mod tests;
