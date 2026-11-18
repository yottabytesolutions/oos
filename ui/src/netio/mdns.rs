// SPDX-License-Identifier: GPL-2.0-or-later
//! Discovery lifecycle on the shared IO executor; the responder remains socket-free.
pub(crate) mod socket;
use super::Runtime;
use crate::mdns::{set_status, Config, Iface, Out, Responder, State, Status};
pub(crate) use socket::{interfaces, ipv4_all};
use std::time::{Duration, Instant};
use tokio::sync::watch;

pub struct Mdns {
    runtime: Runtime,
    control: Option<watch::Sender<Config>>,
    task: Option<super::executor::Task<()>>,
    only: Option<Vec<String>>,
}
impl Mdns {
    pub fn new(runtime: Runtime, only: Option<Vec<String>>) -> Self {
        Self { runtime, control: None, task: None, only }
    }
    pub fn set(&mut self, cfg: Config) {
        if let Some(control) = &self.control {
            control.send_replace(cfg);
        } else if cfg.on {
            let (control, changed) = watch::channel(cfg);
            let only = self.only.clone();
            match self.runtime.start(move |stop| Ok(((), discover(changed, only, stop)))) {
                Ok(task) => {
                    self.task = Some(task);
                    self.control = Some(control);
                }
                Err(error) => eprintln!("oos-ui: mDNS task: {error}"),
            }
        }
    }
    pub fn shutdown(&mut self, deadline: Instant) {
        self.control.take();
        if let Some(mut task) = self.task.take() {
            task.shutdown(deadline);
        }
    }
}
impl Drop for Mdns {
    fn drop(&mut self) {
        self.shutdown(Instant::now() + Duration::from_secs(1));
    }
}

struct Discovery {
    responder: Responder,
    socket: Option<socket::Net>,
    ifaces: Vec<Iface>,
    refresh: f64,
}
impl Discovery {
    fn configure(&mut self, now: f64, cfg: Config) {
        if cfg.on && self.socket.is_none() {
            match socket::Net::open() {
                Ok(s) => {
                    self.socket = Some(s);
                    self.refresh = f64::NEG_INFINITY;
                }
                Err(e) => {
                    eprintln!("oos-ui: mDNS: port 5353: {e}");
                    set_status(Status { state: State::Failed, name: cfg.name });
                    return;
                }
            }
        }
        let on = cfg.on;
        let out = self.responder.set_config(now, cfg);
        self.send(out);
        if !on {
            self.socket = None;
            self.ifaces.clear();
            self.responder.set_ifaces(now, Vec::new());
        }
        self.publish();
    }
    fn send(&self, out: Vec<Out>) {
        if let Some(s) = &self.socket {
            for packet in out {
                s.send(&packet, &self.ifaces);
            }
        }
    }
    fn tick(&mut self, now: f64, only: Option<&[String]>) {
        if let Some(s) = &mut self.socket {
            if now >= self.refresh {
                self.refresh = now + 2.0;
                let new = interfaces(only);
                if new != self.ifaces {
                    s.join(&new);
                    self.ifaces = new;
                    let out = self.responder.set_ifaces(now, self.ifaces.clone());
                    self.send(out);
                }
            }
            let out = self.responder.tick(now);
            self.send(out);
            self.publish();
        }
    }
    fn publish(&mut self) {
        self.responder.flush_log();
        set_status(Status { state: self.responder.state(), name: self.responder.host_label() });
    }
}

async fn discover(
    mut changed: watch::Receiver<Config>,
    only: Option<Vec<String>>,
    stop: tokio_util::sync::CancellationToken,
) {
    let start = tokio::time::Instant::now();
    let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.as_nanos() as u64)
        ^ std::process::id() as u64;
    let mut discovery =
        Discovery { responder: Responder::new(seed), socket: None, ifaces: Vec::new(), refresh: f64::NEG_INFINITY };
    let mut buf = [0; 9000];
    'discovery: loop {
        let cfg = changed.borrow_and_update().clone();
        discovery.configure(start.elapsed().as_secs_f64(), cfg);
        loop {
            let now = start.elapsed().as_secs_f64();
            discovery.tick(now, only.as_deref());
            let wake = discovery.responder.next_wake().unwrap_or(now + 1.0).min(discovery.refresh);
            tokio::select! {
                result = changed.changed() => {
                    if result.is_err() { break 'discovery; }
                    break;
                }
                _ = stop.cancelled() => break 'discovery,
                packet = async {
                    if let Some(s) = &discovery.socket { s.recv(&mut buf).await }
                    else { std::future::pending().await }
                } => {
                    if let Ok((n, src, index, group)) = packet {
                        let out = discovery.responder.receive(start.elapsed().as_secs_f64(), &buf[..n], src, index, group);
                        discovery.send(out);
                    }
                }
                _ = tokio::time::sleep(Duration::from_secs_f64((wake - now).clamp(0.0, 1.0))), if discovery.socket.is_some() => {}
            }
        }
    }
    let out = discovery.responder.stop(start.elapsed().as_secs_f64());
    discovery.send(out);
    set_status(Status::default());
}

#[cfg(test)]
mod tests;
