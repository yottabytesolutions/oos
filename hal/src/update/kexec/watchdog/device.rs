// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

use super::Handoff;
use std::io::{self, Write};
use std::time::{Duration, Instant};

/// The filesystem, process and clock boundary; host tests never touch the real watchdog or supervisor.
pub(super) trait Port {
    type Handle: Write;
    fn present(&self) -> io::Result<bool>;
    fn pid(&self) -> io::Result<String>;
    fn cmdline(&self, pid: libc::pid_t) -> io::Result<Vec<u8>>;
    fn requester(&self) -> io::Result<Option<String>>;
    fn request(&self) -> io::Result<()>;
    fn remove_request(&self) -> io::Result<()>;
    fn signal(&self, pid: libc::pid_t, signal: libc::c_int) -> io::Result<()>;
    fn open(&self) -> io::Result<Self::Handle>;
    fn now(&self) -> Instant;
    fn pause(&self);
    fn report(&self, message: &str);
}

pub(super) struct Device<P: Port> {
    port: P,
    requested: bool,
    file: Option<P::Handle>,
    absent: bool,
}

fn pid(text: &str) -> Result<libc::pid_t, String> {
    text.trim().parse().ok().filter(|&n| n > 1).ok_or_else(|| "invalid watchdog supervisor/requester pid".into())
}

impl<P: Port> Device<P> {
    pub(super) fn new(port: P) -> Self {
        Self { port, requested: false, file: None, absent: false }
    }

    /// Separate from the read-only preflight: only a recorded, provably dead requester can be recovered.
    pub(super) fn recover_stale_request(&self) -> Result<(), String> {
        let Some(request) = self.port.requester().map_err(|e| e.to_string())? else { return Ok(()) };
        let requester = pid(&request).map_err(|_| {
            "leftover /tmp/oos-watchdog.handoff has no valid requester pid; left in place for inspection".to_string()
        })?;
        match self.port.signal(requester, 0) {
            Err(e) if e.raw_os_error() == Some(libc::ESRCH) => {
                self.port.remove_request().map_err(|e| format!("cannot remove stale watchdog handoff: {e}"))?;
                self.port.report("removed leftover /tmp/oos-watchdog.handoff: recorded requester is dead");
                Ok(())
            }
            _ => Err(format!(
                "leftover /tmp/oos-watchdog.handoff: requester {requester} may still be alive; left in place"
            )),
        }
    }

    /// No stops, pings, signals or file writes. Repeat at takeover to reject changes since teardown began.
    pub(super) fn preflight(&self) -> Result<Option<libc::pid_t>, String> {
        if self.port.requester().map_err(|e| e.to_string())?.is_some() {
            return Err("leftover /tmp/oos-watchdog.handoff; watchdog handoff already requested".into());
        }
        if !self.port.present().map_err(|e| format!("watchdog device check: {e}"))? {
            return Ok(None);
        }
        let supervisor = pid(&self.port.pid().map_err(|e| format!("watchdog supervisor pid file: {e}"))?)?;
        let cmd = self.port.cmdline(supervisor).map_err(|e| format!("watchdog supervisor cmdline: {e}"))?;
        if !cmd.split(|&b| b == 0).any(|arg| arg == b"/bin/oos-watchdog") {
            return Err("watchdog supervisor identity changed".into());
        }
        Ok(Some(supervisor))
    }
}

impl<P: Port> Handoff for Device<P> {
    fn take(&mut self) -> Result<(), String> {
        let Some(supervisor) = self.preflight()? else {
            self.absent = true;
            self.port.report("/dev/watchdog absent; proceeding with kexec without a watchdog ping");
            return Ok(());
        };
        self.port.request().map_err(|e| format!("watchdog handoff request: {e}"))?;
        self.requested = true;
        self.port.signal(supervisor, libc::SIGUSR1).map_err(|e| format!("watchdog supervisor signal: {e}"))?;
        let deadline = self.port.now() + Duration::from_secs(2);
        loop {
            match self.port.open() {
                Ok(file) => {
                    self.file = Some(file);
                    return Ok(());
                }
                Err(e) if e.raw_os_error() == Some(libc::EBUSY) && self.port.now() < deadline => self.port.pause(),
                Err(e) => return Err(format!("watchdog takeover: {e}")),
            }
        }
    }

    fn ping(&mut self) -> Result<(), String> {
        if self.absent {
            return Ok(());
        }
        // A non-V write clears magic-close permission and invokes watchdog_ping. dw_wdt has no minimum
        // heartbeat interval, so the core calls its hardware ping synchronously, not a deferred worker.
        self.file.as_mut().ok_or("watchdog not acquired")?.write_all(&[0]).map_err(|e| format!("watchdog ping: {e}"))
    }

    fn restore(&mut self) {
        // No magic close: a stuck restore is still bounded by the watchdog's userspace deadline.
        drop(self.file.take());
        if self.requested {
            match self.port.remove_request() {
                Ok(()) => self.requested = false,
                Err(e) if e.kind() == io::ErrorKind::NotFound => self.requested = false,
                Err(e) => self.port.report(&format!("cannot resume watchdog supervisor: {e}")),
            }
        }
    }
}

impl<P: Port> Drop for Device<P> {
    fn drop(&mut self) {
        self.restore();
    }
}
