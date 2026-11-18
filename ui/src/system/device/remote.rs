// SPDX-License-Identifier: GPL-2.0-or-later
//! Device process wiring, extracted from main.
use crate::*;
use std::time::Instant;

pub(super) struct Remote {
    pub(super) web: http::Http,
    scpi: scpi::Scpi,
    vxi: vxi11::Server,
    hislip: hislip::Server,
    /// the scope (`on_scope`): VXI-11 and HiSLIP follow their switches
    scope: bool,
    web_port: u16,
    scpi_port: u16,
    hislip_port: u16,
    remote_now: Option<[bool; 7]>,
    mdns: mdns::Mdns,
    mdns_now: Option<mdns::Config>,
    mdns_t: Option<Instant>,
}
impl Remote {
    pub(super) fn new(args: &[String], sw: &sysconf::Switches, ui: &mut Ui) -> Self {
        let web_port = arg(args, "--web-port").and_then(|p| p.parse().ok()).unwrap_or(8080);
        let scpi_port = arg(args, "--scpi-port").and_then(|p| p.parse().ok()).unwrap_or(5555);
        let scope = on_scope();
        let mut web = http::Http::open(web_port, sw);
        let mut scpi =
            scpi::Scpi::open_with_runtime(scpi_port, sw.ipv6, net6::host_or_exit(web.io_runtime()), web.wake_handle());
        let mut vxi = vxi11::Server::closed(net6::host_or_exit(scpi.io_runtime()), web.wake_handle());
        vxi.set_open(scope, sw.ipv6);
        let hislip_port = arg(args, "--hislip-port").and_then(|p| p.parse().ok()).unwrap_or(hislip::PORT);
        let mut hislip = hislip::Server::closed_with_runtime(net6::host_or_exit(scpi.io_runtime()), web.wake_handle());
        hislip.set_open((!scope || ui.sys.instruments.hislip).then_some(hislip_port.into()), sw.ipv6);
        // who may update or control the scope over the network: net.allow (docs/network.md 6), read here at the start;
        // the servers share the list with Remote services > Allowed, which sets it after a change
        let acl = ui.acl();
        acl.set(netacl::Acl::load(&sysconf::default_path()));
        if std::path::Path::new(crate::remote::FIREWALL).exists() {
            ui.set_allow_runner(crate::remote::script_runner());
        }
        web.set_acl(acl.clone());
        (scpi.acl, vxi.acl, hislip.acl) = (acl.clone(), acl.clone(), acl);
        let mdns = mdns::Mdns::new(net6::host_or_exit(scpi.io_runtime()), None);
        let mut r = Self {
            web,
            scpi,
            vxi,
            hislip,
            scope,
            web_port,
            scpi_port,
            hislip_port,
            remote_now: None,
            mdns,
            mdns_now: None,
            mdns_t: None,
        };
        r.offer();
        r
    }
    /// The identification page names the listeners that are really up.
    fn offer(&mut self) {
        self.web.offer(http::Offer {
            scpi_port: self.scpi.port(),
            vxi11: self.vxi.service_port().is_some(),
            hislip_port: self.hislip.port(),
        });
    }
    pub(super) fn wake(&self) -> &system::intake::Wake {
        self.web.wake()
    }
    fn switch(&mut self, sys: &sysconf::SysConf, sw: &sysconf::Switches) {
        // the services follow their switches: the ports were opened above, SSH (oos-ssh) and telnet are started by rcS
        let want = [
            sys.remote_web,
            sys.remote_scpi,
            sys.instruments.vxi11,
            sys.remote_ssh,
            sys.ssh_password,
            sys.remote_telnet,
            !self.scope || sys.instruments.hislip,
        ];
        if self.remote_now != Some(want) {
            let was =
                self.remote_now.unwrap_or([true, true, true, want[3], want[4], !want[5], self.hislip.port().is_some()]);
            if want[0] != was[0] {
                self.web.set_open(want[0].then_some(self.web_port), sw.ipv6);
            }
            if want[1] != was[1] {
                self.scpi.set_open(want[1].then_some(self.scpi_port), sw.ipv6);
            }
            // VXI-11 needs port 111, which only the scope has free
            if want[2] != was[2] && self.scope {
                self.vxi.set_open(want[2], sw.ipv6);
            }
            if want[3] != was[3] || want[4] != was[4] {
                ssh::apply(want[3]);
            }
            if want[5] != was[5] {
                remote::telnet(want[5]);
            }
            if want[6] != was[6] {
                self.hislip.set_open(want[6].then_some(self.hislip_port.into()), sw.ipv6);
            }
            self.offer();
            self.remote_now = Some(want);
        }
    }
    pub(super) fn poll(&mut self, ui: &mut Ui, cv: &Canvas, sw: &sysconf::Switches) {
        self.switch(&ui.sys, sw);
        if self.mdns_t.is_none_or(|t| t.elapsed().as_secs() >= 1) {
            self.mdns_t = Some(Instant::now());
            let c = mdns_services(
                ui.mdns_config(self.web_port, self.scpi_port),
                self.web.port(),
                self.vxi.service_port(),
                self.hislip.port(),
            );
            if self.mdns_now.as_ref() != Some(&c) {
                if self.mdns_now.as_ref().is_none_or(|o| o.name != c.name) {
                    hostname::set(&c.name);
                }
                self.mdns.set(c.clone());
                self.mdns_now = Some(c);
            }
        }
        self.scpi.poll(ui, cv);
        self.vxi.poll_async(&mut |program| self.scpi.poll_program(ui, cv, program));
        self.hislip.poll_async(&mut |program| self.scpi.poll_program(ui, cv, program));
        for c in self.web.poll(ui, cv, &mut self.scpi) {
            if let cmd::Out::Shot(p) = cmd::run(ui, &c) {
                ui.shot_request = Some(p);
            }
        }
    }
    pub(super) fn release_buffers(&mut self) {
        self.web.release_buffers();
        self.vxi.release_buffers();
    }

    pub(super) fn close(&mut self, deadline: Instant) {
        self.scpi.set_open(None, false);
        self.vxi.set_open(false, false);
        self.hislip.set_open(None, false);
        self.mdns.shutdown(deadline);
        self.web.shutdown(deadline);
    }
}

/// The scope, where port 111 is free for VXI-11: the acquisition device is present, or the boot identity was read.
/// A computer has neither, and accepts HiSLIP instead.
fn on_scope() -> bool {
    std::path::Path::new("/dev/xdma0_bypass").exists()
        || matches!(oos_hal::model::platform().source, oos_hal::model::Source::Vendor)
}

/// The name responder's services as System > Network sets them, made to match the listeners that are really up:
/// VXI-11 only where it listens, HiSLIP where it does, and LXI discovery (`_lxi._tcp`) beside the web server, both
/// pointing at the identification page.
fn mdns_services(mut c: mdns::Config, web: Option<u16>, vxi: Option<u16>, hislip: Option<u16>) -> mdns::Config {
    if vxi.is_none() {
        c.services.retain(|s| s.kind != "_vxi-11._tcp");
    }
    if let Some(p) = web.filter(|_| c.services.iter().any(|s| s.kind == "_http._tcp")) {
        let path = ("path".into(), Some("/lxi/identification".into()));
        c.services.push(mdns::Service { kind: "_lxi._tcp".into(), port: p, txt: vec![path] });
    }
    if let Some(p) = hislip {
        c.services.push(mdns::Service { kind: "_hislip._tcp".into(), port: p, txt: Vec::new() });
    }
    c
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::Write, net::TcpStream};

    fn announces_hislip(port: Option<u16>) -> bool {
        mdns_services(mdns::Config::default(), None, None, port)
            .services
            .iter()
            .any(|s| s.kind == "_hislip._tcp" && Some(s.port) == port)
    }

    fn scope_remote(sw: &sysconf::Switches) -> Remote {
        let web = http::Http::open(0, sw);
        let mut scpi = scpi::Scpi::open(0, false);
        let runtime = scpi.io_runtime().unwrap();
        let hislip = hislip::Server::closed_with_runtime(runtime.clone(), web.wake_handle());
        let vxi = vxi11::Server::closed(runtime.clone(), web.wake_handle());
        Remote {
            web,
            scpi,
            vxi,
            hislip,
            scope: true,
            web_port: 0,
            scpi_port: 0,
            hislip_port: 0,
            remote_now: Some([true, true, false, false, true, false, false]),
            mdns: mdns::Mdns::new(runtime, None),
            mdns_now: None,
            mdns_t: None,
        }
    }

    #[test]
    fn scope_hislip_switch_closes_and_reopens_listener_and_discovery() {
        let sw = sysconf::Switches::default();
        let mut r = scope_remote(&sw);
        let mut sys = sysconf::SysConf::default();
        sys.instruments.vxi11 = false;
        r.switch(&sys, &sw);
        assert_eq!(r.hislip.port(), None);
        for _ in 0..2 {
            sys.instruments.hislip = true;
            r.switch(&sys, &sw);
            let port = r.hislip.port().unwrap();
            assert!(announces_hislip(Some(port)));
            let mut peer = TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, port)).unwrap();
            peer.write_all(&hislip::encode(0, 0, 0x0100_0000, b"hislip0")).unwrap();
            peer.set_nonblocking(true).unwrap();
            let mut bytes = Vec::new();
            crate::ui::lifecycle::test_wait::until("HiSLIP initialize", || {
                use std::io::Read;
                r.hislip.poll(&mut |_| Vec::new());
                let mut buffer = [0; 32];
                match peer.read(&mut buffer) {
                    Ok(n) => bytes.extend_from_slice(&buffer[..n]),
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                    Err(e) => panic!("{e}"),
                }
                hislip::take_message(&mut bytes).unwrap()
            });
            sys.instruments.hislip = false;
            r.switch(&sys, &sw);
            assert_eq!(r.hislip.port(), None);
            crate::ui::lifecycle::test_wait::until("HiSLIP connection closed", || {
                use std::io::Read;
                match peer.read(&mut [0; 1]) {
                    Ok(0) => Some(()),
                    Err(e) if e.kind() == std::io::ErrorKind::ConnectionReset => Some(()),
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => None,
                    other => panic!("unexpected closed connection: {other:?}"),
                }
            });
            assert!(TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, port)).is_err());
            assert!(!announces_hislip(r.hislip.port()));
        }
    }
}
