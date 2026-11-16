// SPDX-License-Identifier: GPL-2.0-or-later
//! Device process wiring, extracted from main.
use crate::*;
use std::time::Instant;

pub(super) struct Remote {
    pub(super) web: http::Http,
    scpi: scpi::Scpi,
    vxi: vxi11::Server,
    web_port: u16,
    scpi_port: u16,
    remote_now: Option<[bool; 6]>,
    mdns: mdns::Mdns,
    mdns_now: Option<mdns::Config>,
    mdns_t: Option<Instant>,
}
impl Remote {
    pub(super) fn new(args: &[String], sw: &sysconf::Switches) -> Self {
        let web_port = arg(args, "--web-port").and_then(|p| p.parse().ok()).unwrap_or(8080);
        let scpi_port = arg(args, "--scpi-port").and_then(|p| p.parse().ok()).unwrap_or(5555);
        let mut web = http::Http::open(web_port, sw);
        let mut scpi = scpi::Scpi::open(scpi_port, sw.ipv6);
        let mut vxi = vxi11::Server::open(sw.ipv6);
        let acl = netacl::Acl::load(&sysconf::default_path());
        web.acl = acl.clone();
        scpi.acl = acl.clone();
        vxi.acl = acl;
        Self {
            web,
            scpi,
            vxi,
            web_port,
            scpi_port,
            remote_now: None,
            mdns: mdns::Mdns::new(None),
            mdns_now: None,
            mdns_t: None,
        }
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
            if want[2] != was[2] {
                self.vxi.set_open(want[2], sw.ipv6);
            }
            if want[3] != was[3] || want[4] != was[4] {
                ssh::apply(want[3]);
            }
            if want[5] != was[5] {
                remote::telnet(want[5]);
            }
            self.remote_now = Some(want);
        }
        if self.mdns_t.is_none_or(|t| t.elapsed().as_secs() >= 1) {
            self.mdns_t = Some(Instant::now());
            let c = ui.mdns_config(self.web_port, self.scpi_port);
            if self.mdns_now.as_ref() != Some(&c) {
                if self.mdns_now.as_ref().is_none_or(|o| o.name != c.name) {
                    hostname::set(&c.name);
                }
                self.mdns.set(c.clone());
                self.mdns_now = Some(c);
            }
        }
        self.scpi.poll(ui, cv);
        self.vxi.poll(&mut |c| self.scpi.exec_line(ui, cv, c));
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
        self.mdns.shutdown(deadline);
        self.web.shutdown(deadline);
    }
}
