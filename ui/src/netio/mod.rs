// SPDX-License-Identifier: GPL-2.0-or-later
//! Raw TCP framing and output. The application port contains no runtime types.
mod output;
mod runtime;
mod session;
mod timer;
pub use crate::netport::*;
use std::{io, sync::Arc, thread::JoinHandle};

pub struct Raw {
    ports: Arc<Ports>,
    control: tokio::sync::watch::Sender<Option<crate::netacl::Shared>>,
    thread: Option<JoinHandle<()>>,
    port: u16,
}
impl Raw {
    pub fn open(port: impl Into<crate::net6::Port>, v6: bool, wake: Arc<Wake>) -> io::Result<Self> {
        let ports = Ports::new(wake)?;
        let (control, changed) = tokio::sync::watch::channel(None);
        let (ready, started) = std::sync::mpsc::sync_channel(1);
        let io_ports = ports.clone();
        let port = port.into();
        let thread = std::thread::Builder::new().name("netio".into()).spawn(move || {
            runtime::run(port, v6, io_ports, changed, ready);
        })?;
        match started.recv().map_err(io::Error::other)? {
            Ok(port) => Ok(Self { ports, control, thread: Some(thread), port }),
            Err(e) => {
                let _ = thread.join();
                Err(e)
            }
        }
    }
    pub fn port(&self) -> u16 {
        self.port
    }
    pub fn ports(&self) -> Arc<Ports> {
        self.ports.clone()
    }
    pub fn set_acl(&self, acl: crate::netacl::Shared) {
        self.control.send_replace(Some(acl));
    }
}
impl Drop for Raw {
    fn drop(&mut self) {
        self.ports.close();
        // Closing the sole control sender wakes the IO loop and aborts its owned connections.
        let (replacement, _) = tokio::sync::watch::channel(None);
        drop(std::mem::replace(&mut self.control, replacement));
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests;
