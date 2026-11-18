// SPDX-License-Identifier: GPL-2.0-or-later
//! Network framing and output behind a synchronous owned application port.
mod endpoint;
mod executor;
pub(crate) mod http;
pub(crate) mod mdns;
pub(crate) mod port;
pub use endpoint::Endpoint;
mod output;
mod rpc;
pub use executor::Runtime;
mod listener;
mod runtime;
mod session;
mod timer;
use port::{Envelope, Id, Ports, Reply, Wake, MAX_CLIENTS};
use std::{io, sync::Arc};

#[derive(Clone, Copy)]
pub(crate) enum Protocol {
    Raw,
    Hislip,
    Rpc(port::rpc::Channel),
}
pub(super) struct Bound {
    core: u16,
    vxi: Option<VxiPorts>,
}
pub(super) struct VxiPorts {
    tcp: u16,
    udp: u16,
}

pub struct Service {
    ports: Arc<Ports>,
    control: tokio::sync::watch::Sender<Option<crate::netacl::Shared>>,
    task: executor::Task<Bound>,
}
impl Service {
    pub(crate) fn start(
        port: crate::net6::Port,
        v6: bool,
        wake: Arc<Wake>,
        runtime: Runtime,
        protocol: Protocol,
    ) -> io::Result<Self> {
        let ports = if matches!(protocol, Protocol::Rpc(_)) {
            Ports::bounded(wake, MAX_CLIENTS + 1, port::rpc::REPLY_BYTES)?
        } else if matches!(protocol, Protocol::Hislip) {
            Ports::bounded(wake, MAX_CLIENTS, MAX_CLIENTS * port::hislip::REPLY_BYTES)?
        } else {
            Ports::new(wake)?
        };
        let (control, changed) = tokio::sync::watch::channel(None);
        let io_ports = ports.clone();
        let task = runtime.start(move |stop| runtime::open(port, v6, io_ports, changed, protocol, stop))?;
        Ok(Self { ports, control, task })
    }
    pub fn port(&self) -> u16 {
        self.task.local.core
    }
    pub fn vxi_ports(&self) -> Option<(u16, u16, u16)> {
        self.task.local.vxi.as_ref().map(|ports| (ports.tcp, ports.udp, self.task.local.core))
    }
    pub fn ports(&self) -> Arc<Ports> {
        self.ports.clone()
    }
    pub fn set_acl(&self, acl: crate::netacl::Shared) {
        self.control.send_replace(Some(acl));
    }
}
#[cfg(test)]
mod tests;
