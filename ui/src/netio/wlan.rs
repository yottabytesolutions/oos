// SPDX-License-Identifier: GPL-2.0-or-later
//! WLAN control on the shared IO executor, with bounded owned menu messages.
mod control;
mod operations;
use super::Runtime;
use crate::netport::wlan::*;
use oos_hal::wlan;
use std::time::Duration;
use tokio::sync::mpsc;

pub(crate) struct Wlan {
    runtime: Runtime,
    port: Port,
    jobs: Option<mpsc::Sender<Job>>,
    reports: Option<mpsc::Receiver<Report>>,
}
impl Wlan {
    pub(crate) fn new(runtime: Runtime, port: Port) -> Self {
        Self { runtime, port, jobs: None, reports: None }
    }
    pub(crate) fn start(&mut self) {
        if self.jobs.is_some() {
            return;
        }
        // One queued action and two reports, plus one executing action/report. Each control reply is <= 16 KiB.
        let (send, jobs) = mpsc::channel(1);
        let (out, reports) = mpsc::channel(2);
        let port = self.port.clone();
        self.runtime.spawn(run(jobs, out, port));
        self.jobs = Some(send);
        self.reports = Some(reports);
    }
    pub(crate) fn send(&self, job: Job) -> bool {
        let job = match job {
            Job::Join(ssid, how) => {
                if ssid.len() > wlan::SSID_MAX {
                    return false;
                }
                let how = match how {
                    How::Pass(pass) if pass.len() <= wlan::PASS_LEN.1 => How::Pass(pass.into_boxed_str().into_string()),
                    How::Pass(_) => return false,
                    how => how,
                };
                Job::Join(ssid.into_boxed_slice().into_vec(), how)
            }
            job => job,
        };
        self.jobs.as_ref().is_some_and(|jobs| jobs.try_send(job).is_ok())
    }
    pub(crate) fn poll(&mut self) -> Option<Report> {
        self.reports.as_mut()?.try_recv().ok()
    }
}

async fn publish(out: &mpsc::Sender<Report>, port: &Port, report: Report) -> bool {
    if out.send(report).await.is_err() {
        return false;
    }
    port.notify();
    true
}

async fn run(mut jobs: mpsc::Receiver<Job>, out: mpsc::Sender<Report>, port: Port) {
    let clock = operations::Timer::new();
    loop {
        if !publish(&out, &port, Report::Link(operations::link().await)).await {
            return;
        }
        let job = tokio::select! {
            job = jobs.recv() => match job { Some(job) => job, None => return },
            () = tokio::time::sleep(Duration::from_secs(2)) => continue,
            () = out.closed() => return,
        };
        // Complete accepted operations, including re-enabling saved networks and syncing successful changes.
        let report = operations::perform(job, &clock, &port).await;
        if !publish(&out, &port, report).await {
            return;
        }
    }
}

#[cfg(test)]
pub(crate) mod tests;
