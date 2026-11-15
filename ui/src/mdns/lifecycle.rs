// SPDX-License-Identifier: GPL-2.0-or-later
//! Bounded responder teardown.
use super::*;
impl Mdns {
    pub fn shutdown(&mut self, deadline: std::time::Instant) {
        if let Some(tx) = self.tx.take() {
            let _ = tx.send(Ctl::Stop);
        }
        if let Some(h) = self.th.take() {
            crate::system::shutdown::join(vec![h], deadline);
        }
    }
}
