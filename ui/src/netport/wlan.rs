// SPDX-License-Identifier: GPL-2.0-or-later
//! Owned WLAN requests and status values; no executor types cross this port.
use oos_hal::wlan::{Net, Saved, Status};

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum How {
    /// a network already saved
    Saved(u32),
    Open,
    Pass(String),
}

pub(crate) enum Job {
    Scan,
    Join(Vec<u8>, How),
    Forget(u32),
}

pub(crate) enum Report {
    Link(Link),
    Scan(Result<Vec<Net>, String>),
    Done(Result<String, String>),
}

/// What the worker last saw.
#[derive(Clone, Debug, Default)]
pub(crate) struct Link {
    /// wlan0 exists: a stick with a working driver
    stick: bool,
    /// wpa_supplicant answers
    ctrl: bool,
    status: Status,
    signal: Option<i32>,
    saved: Vec<Saved>,
}

impl Link {
    pub(crate) fn new(stick: bool, ctrl: bool, status: Status, signal: Option<i32>, saved: Vec<Saved>) -> Self {
        Self { stick, ctrl, status, signal, saved }
    }
    pub(crate) fn stick(&self) -> bool {
        self.stick
    }
    pub(crate) fn ctrl(&self) -> bool {
        self.ctrl
    }
    pub(crate) fn status(&self) -> &Status {
        &self.status
    }
    pub(crate) fn signal(&self) -> Option<i32> {
        self.signal
    }
    pub(crate) fn saved(&self) -> &[Saved] {
        &self.saved
    }
}

/// Frame notification and durable-save operation supplied by the application.
#[derive(Clone)]
pub(crate) struct Port {
    wake: std::sync::Arc<super::Wake>,
    sync_disks: fn(),
}
impl Port {
    pub(crate) fn new(wake: std::sync::Arc<super::Wake>, sync_disks: fn()) -> Self {
        Self { wake, sync_disks }
    }
    pub(crate) fn notify(&self) {
        self.wake.notify();
    }
    pub(crate) fn sync_disks(&self) {
        (self.sync_disks)();
    }
}
