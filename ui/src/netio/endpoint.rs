// SPDX-License-Identifier: GPL-2.0-or-later
//! Frame-owned service switching and ACL publication shared by transports.
use super::*;

pub struct Endpoint {
    pub(crate) io: Option<Service>,
    runtime: Option<Runtime>,
    pub(crate) wake: Arc<Wake>,
    acl_seen: u64,
}
impl Default for Endpoint {
    fn default() -> Self {
        Self::new(None, Arc::new(Wake::new().expect("remote intake requires a wake descriptor")))
    }
}
impl Endpoint {
    pub fn new(runtime: Option<Runtime>, wake: Arc<Wake>) -> Self {
        Self { io: None, runtime, wake, acl_seen: u64::MAX }
    }
    pub fn runtime(&mut self) -> io::Result<Runtime> {
        if self.runtime.is_none() {
            self.runtime = Some(Runtime::new()?);
        }
        self.runtime.clone().ok_or_else(|| io::Error::other("IO runtime unavailable"))
    }
    pub(crate) fn set_open(&mut self, port: Option<crate::net6::Port>, v6: bool, protocol: Protocol) {
        match port {
            Some(port) if self.io.is_none() => {
                self.io = self
                    .runtime()
                    .and_then(|runtime| Service::start(port, v6, self.wake.clone(), runtime, protocol))
                    .ok();
                self.acl_seen = u64::MAX;
            }
            Some(_) => {}
            None => self.io = None,
        }
    }
    pub fn port(&self) -> Option<u16> {
        self.io.as_ref().map(Service::port)
    }
    pub fn ports(&mut self, acl: &crate::netacl::Shared) -> Option<Arc<Ports>> {
        let io = self.io.as_ref()?;
        if acl.changed(&mut self.acl_seen) {
            io.set_acl(acl.clone());
        }
        Some(io.ports())
    }
}
