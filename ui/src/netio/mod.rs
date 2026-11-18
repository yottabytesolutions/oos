// SPDX-License-Identifier: GPL-2.0-or-later
//! Network framing and output. The application port contains no runtime types.
mod endpoint;
mod executor;
pub(crate) mod http;
pub use endpoint::Endpoint;
mod output;
mod rpc;
pub use executor::Runtime;
mod runtime;
mod session;
mod timer;
pub use crate::netport::*;
use std::{io, sync::Arc};

#[derive(Clone, Copy)]
pub(crate) enum Protocol {
    Raw,
    Hislip,
    Rpc(crate::netport::rpc::Channel),
}
pub(super) type BoundPorts = (u16, Option<(u16, u16)>);

pub struct Service {
    ports: Arc<Ports>,
    control: tokio::sync::watch::Sender<Option<crate::netacl::Shared>>,
    stopped: std::sync::mpsc::Receiver<()>,
    _runtime: Runtime,
    bound: BoundPorts,
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
            Ports::bounded(wake, MAX_CLIENTS + 1, crate::netport::rpc::REPLY_BYTES)?
        } else if matches!(protocol, Protocol::Hislip) {
            Ports::bounded(wake, MAX_CLIENTS, MAX_CLIENTS * crate::netport::hislip::REPLY_BYTES)?
        } else {
            Ports::new(wake)?
        };
        let (control, changed) = tokio::sync::watch::channel(None);
        let (ready, started) = std::sync::mpsc::sync_channel(1);
        let (finished, stopped) = std::sync::mpsc::sync_channel(1);
        let io_ports = ports.clone();
        runtime.spawn(async move {
            runtime::run(port, v6, io_ports, changed, ready, protocol).await;
            let _ = finished.send(());
        });
        let bound = started.recv().map_err(io::Error::other)??;
        Ok(Self { ports, control, stopped, _runtime: runtime, bound })
    }
    pub fn port(&self) -> u16 {
        self.bound.0
    }
    pub fn vxi_ports(&self) -> Option<(u16, u16, u16)> {
        self.bound.1.map(|(tcp, udp)| (tcp, udp, self.bound.0))
    }
    pub fn ports(&self) -> Arc<Ports> {
        self.ports.clone()
    }
    pub fn set_acl(&self, acl: crate::netacl::Shared) {
        self.control.send_replace(Some(acl));
    }
}
impl Drop for Service {
    fn drop(&mut self) {
        self.ports.close();
        // Closing the sole control sender wakes the IO loop and aborts its owned connections.
        let (replacement, _) = tokio::sync::watch::channel(None);
        drop(std::mem::replace(&mut self.control, replacement));
        let _ = self.stopped.recv();
    }
}

#[cfg(test)]
mod tests;
