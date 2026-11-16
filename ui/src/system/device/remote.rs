// SPDX-License-Identifier: GPL-2.0-or-later
//! Device process wiring, extracted from main.
use crate::*;
use std::time::Instant;

pub(super) struct Remote {
    pub(super) web: http::Http,
    scpi: scpi::Scpi,
    vxi: vxi11::Server,
    /// HiSLIP: a computer only; the scope offers VXI-11 instead (port 111 is free there)
    hislip: hislip::Server,
    /// the scope (`on_scope`): VXI-11 follows its switch, HiSLIP stays closed
    scope: bool,
    web_port: u16,
    scpi_port: u16,
    remote_now: Option<[bool; 6]>,
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
        let mut scpi = scpi::Scpi::open(scpi_port, sw.ipv6);
        let mut vxi = if scope { vxi11::Server::open(sw.ipv6) } else { vxi11::Server::closed() };
        let hislip = if scope {
            hislip::Server::closed()
        } else {
            let port = arg(args, "--hislip-port").and_then(|p| p.parse().ok()).unwrap_or(hislip::PORT);
            hislip::Server::open(port)
        };
        // who may update or control the scope over the network: net.allow (docs/network.md 6), read here at the start;
        // the servers share the list with Remote services > Allowed, which sets it after a change
        let acl = ui.acl();
        acl.set(netacl::Acl::load(&sysconf::default_path()));
        if std::path::Path::new(crate::remote::FIREWALL).exists() {
            ui.set_allow_runner(crate::remote::script_runner());
        }
        web.set_acl(acl.clone());
        (scpi.acl, vxi.acl) = (acl.clone(), acl);
        let mut r = Self {
            web,
            scpi,
            vxi,
            hislip,
            scope,
            web_port,
            scpi_port,
            remote_now: None,
            mdns: mdns::Mdns::new(None),
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
    pub(super) fn poll(&mut self, ui: &mut Ui, cv: &Canvas, sw: &sysconf::Switches) {
        // the services follow their switches: the ports were opened above, SSH (oos-ssh) and telnet are started by rcS
        let want = [
            ui.sys.remote_web,
            ui.sys.remote_scpi,
            ui.sys.remote_vxi11,
            ui.sys.remote_ssh,
            ui.sys.ssh_password,
            ui.sys.remote_telnet,
        ];
        if self.remote_now != Some(want) {
            let was = self.remote_now.unwrap_or([true, true, true, want[3], want[4], !want[5]]);
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
            self.offer();
            self.remote_now = Some(want);
        }
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
        self.hislip = hislip::Server::closed();
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
        let path = format!("path={}", announce::IDENTIFICATION_PATH);
        c.services.push(mdns::Service { kind: "_lxi._tcp".into(), port: p, txt: vec![path] });
    }
    if let Some(p) = hislip {
        c.services.push(mdns::Service { kind: "_hislip._tcp".into(), port: p, txt: Vec::new() });
    }
    c
}
