// SPDX-License-Identifier: GPL-2.0-or-later
//! Web listeners use the same A53 idle policy as PNG and send workers.
use super::*;

impl Http {
    pub fn wake_handle(&self) -> Arc<crate::system::intake::Wake> {
        self.inbox.port.wake_handle()
    }

    pub(super) fn start(&mut self, port: impl Into<crate::net6::Port>, v6: bool) {
        let Some(l) = crate::net6::tcp(port, v6) else { return };
        let Ok(addr) = l.local_addr() else { return };
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (inbox, acl, st, lim) = (self.inbox.clone(), self.acl_shared.clone(), stop.clone(), self.limits);
        let (tasks, little) = (self.shots.tasks.clone(), self.shots.little);
        if self
            .shots
            .tasks
            .spawn("web-accept", None, move || {
                lower_priority(little);
                acceptor(l, inbox, acl, st, lim, tasks)
            })
            .is_err()
        {
            eprintln!("oos-ui: web remote: no accept thread");
            return;
        }
        eprintln!("oos-ui: web remote on {}", addr);
        self.open = Some((addr, stop));
    }
}
