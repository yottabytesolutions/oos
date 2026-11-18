// SPDX-License-Identifier: GPL-2.0-or-later
//! Owned discovery configuration on netio; mdns-sd owns the responder thread and sockets.
use super::Runtime;
use crate::mdns::{set_status, Config, State, Status};
use core::net::IpAddr;
use mdns_sd::{DaemonEvent, DaemonStatus, IfKind, ServiceDaemon, ServiceInfo, TxtProperty};
use std::{
    collections::HashSet,
    fmt,
    sync::mpsc,
    time::{Duration, Instant},
};
use tokio::sync::watch;

const DAEMON_TIMEOUT: Duration = Duration::from_millis(500);
const RETRY_DELAY: Duration = Duration::from_secs(2);
// mdns-sd appends a u32 conflict counter without shortening the original label.
const CONFLICT_DIGITS: usize = u32::MAX.ilog10() as usize + 1;

#[derive(Debug)]
enum DiscoveryError {
    Daemon(mdns_sd::Error),
    Timeout,
    Closed,
    NotRunning,
}
impl From<mdns_sd::Error> for DiscoveryError {
    fn from(error: mdns_sd::Error) -> Self {
        Self::Daemon(error)
    }
}
impl From<tokio::time::error::Elapsed> for DiscoveryError {
    fn from(_: tokio::time::error::Elapsed) -> Self {
        Self::Timeout
    }
}
impl fmt::Display for DiscoveryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Daemon(error) => error.fmt(f),
            Self::Timeout => f.write_str("daemon reply timed out"),
            Self::Closed => f.write_str("daemon channel closed"),
            Self::NotRunning => f.write_str("daemon is not running"),
        }
    }
}
impl std::error::Error for DiscoveryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Daemon(error) => Some(error),
            _ => None,
        }
    }
}

pub struct Mdns {
    runtime: Runtime,
    control: Option<watch::Sender<Config>>,
    stopped: Option<mpsc::Receiver<()>>,
    only: Option<Vec<String>>,
}
impl Mdns {
    pub fn new(runtime: Runtime, only: Option<Vec<String>>) -> Self {
        Self { runtime, control: None, stopped: None, only }
    }
    pub fn set(&mut self, cfg: Config) {
        if let Some(control) = &self.control {
            control.send_replace(cfg);
        } else if cfg.on {
            let (control, changed) = watch::channel(cfg);
            let (finished, stopped) = mpsc::sync_channel(1);
            let only = self.only.clone();
            self.runtime.spawn(async move {
                run(changed, only).await;
                let _ = finished.send(());
            });
            self.control = Some(control);
            self.stopped = Some(stopped);
        }
    }
    pub fn shutdown(&mut self, deadline: Instant) {
        self.control.take();
        if let Some(stopped) = self.stopped.take() {
            let _ = stopped.recv_timeout(deadline.saturating_duration_since(Instant::now()));
        }
    }
}
impl Drop for Mdns {
    fn drop(&mut self) {
        self.shutdown(Instant::now() + Duration::from_secs(1));
    }
}

fn label_prefix(label: &str, max: usize) -> &str {
    &label[..label.floor_char_boundary(max)]
}

fn registration(cfg: &Config, service: &crate::mdns::Service) -> mdns_sd::Result<ServiceInfo> {
    let host = label_prefix(&cfg.name, crate::mdns::NAME_MAX - CONFLICT_DIGITS - 1).trim_end_matches('-');
    let instance = label_prefix(&cfg.instance, crate::mdns::NAME_MAX - CONFLICT_DIGITS - 3);
    let properties: Vec<TxtProperty> = service
        .txt
        .iter()
        .map(|(key, value)| match value {
            Some(value) => (key, value).into(),
            None => key.as_str().into(),
        })
        .collect();
    ServiceInfo::new(
        &format!("{}.local.", service.kind),
        instance,
        &format!("{host}.local."),
        "",
        service.port,
        properties,
    )
    .map(ServiceInfo::enable_addr_auto)
}

// Called once on a fresh daemon. Config changes replace it, restoring default IPv6 selection.
fn configure(daemon: &ServiceDaemon, cfg: &Config, only: Option<&[String]>) -> mdns_sd::Result<()> {
    daemon.set_ip_check_interval(2)?;
    if let Some(names) = only {
        daemon.disable_interface(IfKind::All)?;
        for name in names {
            daemon.enable_interface(name.as_str())?;
        }
    } else {
        daemon.disable_interface(vec![IfKind::LoopbackV4, IfKind::LoopbackV6])?;
    }
    if !cfg.v6 {
        daemon.disable_interface(IfKind::IPv6)?;
    }
    for service in &cfg.services {
        daemon.register(registration(cfg, service)?)?;
    }
    Ok(())
}

struct Discovery {
    name: String,
    joined: HashSet<IpAddr>,
    announced: bool,
}
impl Discovery {
    fn status(&self) -> Status {
        let state = if self.joined.is_empty() {
            State::NoNetwork
        } else if self.announced {
            State::Running
        } else {
            State::Probing
        };
        Status { state, name: self.name.clone() }
    }

    fn update_addresses(&mut self, event: &DaemonEvent, mut joined: HashSet<IpAddr>) {
        // Apply removal even when the snapshot still contains the address or earlier removals.
        if let DaemonEvent::IpDel(ip) = event {
            joined.retain(|addr| self.joined.contains(addr));
            joined.remove(ip);
        }
        if self.joined.is_empty() || joined.is_empty() {
            self.announced = false;
        }
        self.joined = joined;
    }

    fn event(&mut self, event: DaemonEvent) -> Result<(), DiscoveryError> {
        match event {
            DaemonEvent::Announce(_, address) => {
                if let Some(host) = announced_host(&address) {
                    self.name = host.into();
                }
                self.announced = true;
            }
            DaemonEvent::NameChange(change) => {
                if matches!(change.rr_type, mdns_sd::RRType::A | mdns_sd::RRType::AAAA) {
                    self.name = host_label(&change.new_name).into();
                }
                self.announced = false;
                eprintln!("oos-ui: mDNS: {} is taken, now {}", change.original, change.new_name);
            }
            DaemonEvent::Error(error) => return Err(error.into()),
            _ => {}
        }
        Ok(())
    }
}

fn host_label(fqdn: &str) -> &str {
    fqdn.strip_suffix(".local.").unwrap_or(fqdn)
}

/// Probe completion carries host:interface; immediate registration carries an address list.
fn announced_host(address: &str) -> Option<&str> {
    if address.starts_with('[') {
        return None;
    }
    let (host, interface) = address.split_once(':')?;
    (host.ends_with(".local.") && !interface.is_empty()).then(|| host_label(host))
}

async fn on_event(daemon: &ServiceDaemon, discovery: &mut Discovery, event: DaemonEvent) -> Result<(), DiscoveryError> {
    if matches!(event, DaemonEvent::IpAdd(_) | DaemonEvent::IpDel(_)) {
        discovery.update_addresses(&event, joined_addresses(daemon).await?);
    }
    discovery.event(event)
}

async fn daemon_reply<T>(reply: mdns_sd::Result<mdns_sd::Receiver<T>>) -> Result<T, DiscoveryError> {
    tokio::time::timeout(DAEMON_TIMEOUT, reply?.recv_async()).await?.map_err(|_| DiscoveryError::Closed)
}

async fn joined_addresses(daemon: &ServiceDaemon) -> Result<HashSet<IpAddr>, DiscoveryError> {
    match daemon_reply(daemon.status()).await? {
        DaemonStatus::Running(state) => Ok(state
            .enabled_intfs()
            .iter()
            .flat_map(|i| [i.ipv4(), i.ipv6()])
            .flatten()
            .filter(|m| m.is_joined())
            .flat_map(|m| m.addrs().iter().copied())
            .collect()),
        _ => Err(DiscoveryError::NotRunning),
    }
}

async fn advertise(
    daemon: &ServiceDaemon,
    cfg: &Config,
    only: Option<&[String]>,
    changed: &mut watch::Receiver<Config>,
) -> Result<bool, DiscoveryError> {
    let events = daemon.monitor()?;
    configure(daemon, cfg, only)?;
    until_changed(changed, observe(daemon, cfg, events)).await
}

async fn until_changed(
    changed: &mut watch::Receiver<Config>,
    discovery: impl std::future::Future<Output = Result<(), DiscoveryError>>,
) -> Result<bool, DiscoveryError> {
    tokio::select! {
        update = changed.changed() => Ok(update.is_ok()),
        result = discovery => result.map(|()| false),
    }
}

async fn observe(
    daemon: &ServiceDaemon,
    cfg: &Config,
    events: mdns_sd::Receiver<DaemonEvent>,
) -> Result<(), DiscoveryError> {
    let mut discovery = Discovery { name: cfg.name.clone(), joined: joined_addresses(daemon).await?, announced: false };
    set_status(discovery.status());
    loop {
        let event = events.recv_async().await.map_err(|_| DiscoveryError::Closed)?;
        on_event(daemon, &mut discovery, event).await?;
        set_status(discovery.status());
    }
}

async fn run(changed: watch::Receiver<Config>, only: Option<Vec<String>>) {
    run_with(changed, only, ServiceDaemon::new, set_status).await;
}

async fn run_daemon(
    daemon: ServiceDaemon,
    cfg: &Config,
    only: Option<&[String]>,
    changed: &mut watch::Receiver<Config>,
) -> Result<bool, DiscoveryError> {
    let result = advertise(&daemon, cfg, only, changed).await;
    // Shutdown withdraws all registrations before dropping the multicast sockets.
    let _ = daemon_reply(daemon.shutdown()).await;
    result
}

async fn retry(changed: &mut watch::Receiver<Config>) -> bool {
    tokio::select! {
        update = changed.changed() => update.is_ok(),
        _ = tokio::time::sleep(RETRY_DELAY) => true,
    }
}

async fn run_with(
    mut changed: watch::Receiver<Config>,
    only: Option<Vec<String>>,
    mut create: impl FnMut() -> mdns_sd::Result<ServiceDaemon>,
    mut publish: impl FnMut(Status),
) {
    loop {
        let cfg = changed.borrow_and_update().clone();
        if cfg.on && !cfg.services.is_empty() {
            let result = match create() {
                Ok(daemon) => run_daemon(daemon, &cfg, only.as_deref(), &mut changed).await,
                Err(error) => Err(error.into()),
            };
            match result {
                Ok(true) => continue,
                Ok(false) => break,
                Err(error) => {
                    eprintln!("oos-ui: mDNS: {error}");
                    publish(Status { state: State::Failed, name: cfg.name });
                    if !retry(&mut changed).await {
                        break;
                    }
                    continue;
                }
            }
        }
        publish(Status::default());
        if changed.changed().await.is_err() {
            break;
        }
    }
    publish(Status::default());
}

#[cfg(test)]
mod tests;
