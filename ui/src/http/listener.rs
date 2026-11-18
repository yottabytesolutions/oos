// SPDX-License-Identifier: GPL-2.0-or-later
//! Web service lifetime on the shared IO executor.
use super::*;
impl Http {
    pub fn wake_handle(&self) -> Arc<crate::system::intake::Wake> {
        self.inbox.ports.requests().wake_handle()
    }
    pub fn io_runtime(&mut self) -> std::io::Result<crate::netio::Runtime> {
        if self.runtime.is_none() {
            self.runtime = Some(crate::netio::Runtime::new()?);
        }
        self.runtime.clone().ok_or_else(|| std::io::Error::other("IO runtime unavailable"))
    }
    pub(super) fn start(&mut self, port: impl Into<crate::net6::Port>, v6: bool) {
        let Ok(runtime) = self.io_runtime() else { return };
        self.open = crate::netio::http::Service::start(
            port.into(),
            v6,
            &runtime,
            self.inbox.clone(),
            self.acl_shared.clone(),
            self.limits,
        )
        .ok();
    }
}
