// SPDX-License-Identifier: GPL-2.0-or-later
//! Bound listener addresses for host readiness.
use super::*;
impl Server {
    pub fn release_buffers(&mut self) {
        for link in self.links.iter_mut().flatten() {
            link.out = Vec::new();
        }
    }

    pub fn ports(&self) -> Option<(u16, u16, u16)> {
        Some((
            self.pm_tcp.as_ref()?.local_addr().ok()?.port(),
            self.pm_udp.as_ref()?.local_addr().ok()?.port(),
            self.core_tcp.as_ref()?.local_addr().ok()?.port(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_reset_releases_replies_and_shutdown_releases_links() {
        let mut server = Server::empty();
        server.links.push(Some(Link { out: vec![42; 4096] }));
        server.release_buffers();
        assert_eq!(server.links.len(), 1);
        assert_eq!(server.links[0].as_ref().unwrap().out.capacity(), 0);
        server.set_open(false, false);
        assert!(server.links.is_empty());
    }
}
