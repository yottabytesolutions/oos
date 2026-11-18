// SPDX-License-Identifier: GPL-2.0-or-later
use super::{control::Ctrl, wlan, Job, Link, Report};
use crate::netport::wlan::{How, Port};
use std::{path::PathBuf, time::Duration};

pub(super) trait Clock {
    fn now(&self) -> Duration;
    async fn sleep(&self, duration: Duration);
}
pub(super) struct Timer(tokio::time::Instant);
impl Timer {
    pub(super) fn new() -> Self {
        Self(tokio::time::Instant::now())
    }
}
impl Clock for Timer {
    fn now(&self) -> Duration {
        self.0.elapsed()
    }
    async fn sleep(&self, duration: Duration) {
        tokio::time::sleep(duration).await;
    }
}

async fn open() -> Result<Ctrl, String> {
    if !stick_there().await {
        return Err("No WLAN stick found".into());
    }
    Ctrl::open(wlan::CTRL.into()).await.map_err(|_| "WLAN is not ready yet".to_string())
}

async fn stick_there() -> bool {
    tokio::task::spawn_blocking(|| PathBuf::from("/sys/class/net").join(wlan::IFACE).exists()).await.unwrap_or(false)
}

pub(super) async fn link() -> Link {
    let link = Link::new(stick_there().await, false, wlan::Status::default(), None, vec![]);
    if !link.stick() {
        return link;
    }
    let Ok(c) = Ctrl::open(wlan::CTRL.into()).await else { return link };
    let link = read_link(&c).await;
    c.close().await;
    link
}

pub(super) async fn read_link(c: &Ctrl) -> Link {
    let Ok(status) = c.status().await else {
        return Link::new(true, false, wlan::Status::default(), None, vec![]);
    };
    let signal = if status.connected() { c.signal().await } else { None };
    let saved = c.networks().await.unwrap_or_default();
    Link::new(true, true, status, signal, saved)
}

pub(super) async fn perform(job: Job, clock: &impl Clock, port: &Port) -> Report {
    let scan = matches!(job, Job::Scan);
    let result = async {
        let c = open().await?;
        let result = match job {
            Job::Scan => self::scan(&c, clock).await.map(|nets| Report::Scan(Ok(nets))),
            Job::Join(ssid, how) => join(&c, ssid, how, clock).await.map(|s| Report::Done(Ok(s))),
            Job::Forget(id) => c.remove(id).await.map(|()| Report::Done(Ok("Network forgotten".into()))),
        };
        let result = match result {
            Ok(Report::Done(message)) => saved_to_card(&c, port.clone()).await.map(|()| Report::Done(message)),
            other => other,
        };
        c.close().await;
        result
    };
    match result.await {
        Ok(report) => report,
        Err(e) if scan => Report::Scan(Err(e)),
        Err(e) => Report::Done(Err(e)),
    }
}

pub(super) async fn scan(c: &Ctrl, clock: &impl Clock) -> Result<Vec<wlan::Net>, String> {
    c.scan().await?;
    // Keep the existing radio scan interval and single retry for an initially empty list.
    clock.sleep(Duration::from_secs(4)).await;
    let mut nets = c.scan_results().await.map_err(|e| e.to_string())?;
    if nets.is_empty() {
        clock.sleep(Duration::from_secs(3)).await;
        nets = c.scan_results().await.map_err(|e| e.to_string())?;
    }
    Ok(nets)
}

pub(super) async fn saved_to_card(c: &Ctrl, port: Port) -> Result<(), String> {
    c.save().await?;
    // The scope is usually switched off at the mains. Never sync disks on the IO executor.
    tokio::task::spawn_blocking(move || port.sync_disks()).await.map_err(|e| e.to_string())
}

/// Joins a network and keeps it only if that worked.
pub(super) async fn join(c: &Ctrl, ssid: Vec<u8>, how: How, clock: &impl Clock) -> Result<String, String> {
    let name = wlan::name(&ssid);
    let (id, new) = match &how {
        How::Saved(id) => (*id, false),
        How::Open => (c.add(&ssid, None).await?, true),
        How::Pass(pass) => {
            let pass = pass.clone();
            let salt = ssid.clone();
            let key = tokio::task::spawn_blocking(move || wlan::psk(&pass, &salt)).await.map_err(|e| e.to_string())?;
            (c.add(&ssid, Some(&key)).await?, true)
        }
    };
    let wrong = if how == How::Open { "no answer" } else { "wrong password?" };
    c.select(id).await?;
    let end = clock.now() + Duration::from_secs(30);
    let done = loop {
        clock.sleep(Duration::from_millis(500)).await;
        let st = c.status().await.map_err(|e| e.to_string())?;
        if st.connected() && st.ssid == ssid {
            break true;
        }
        let gave_up = c.networks().await.is_ok_and(|v| v.iter().any(|n| n.id == id && n.temp_disabled));
        if gave_up || clock.now() > end {
            break false;
        }
    };
    // SELECT_NETWORK switched the other saved networks off.
    c.enable_all().await?;
    if !done {
        if new {
            c.remove(id).await?;
        }
        return Err(format!("Could not join {}: {}", name, wrong));
    }
    Ok(format!("Connected to {}", name))
}
