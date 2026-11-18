// SPDX-License-Identifier: GPL-2.0-or-later

pub(crate) mod kernel;
mod planes;
pub(crate) mod pool;
pub(crate) mod standard;
pub(crate) use planes::{DiffBand, DiffPlane};
#[cfg(test)]
mod tests;

use crate::drawpipe::Counters;
use crate::model::NCH;
use oos_core::change::Identity;
use std::sync::Arc;

pub(crate) struct Deposit {
    gen: u64,
    size: (usize, usize),
    pub(crate) identity: Option<Identity>,
    pub(crate) records: u64,
    pub(crate) triggered: u64,
    pub(crate) planes: Vec<DiffBand>,
    pub(crate) folds: crate::framestats::Folds,
    pub(crate) edges: [u64; 4],
    pub(crate) off: (f64, f64),
    pub(crate) newest: Option<Arc<crate::board::Frame>>,
    pub(crate) read_at: Option<std::time::Instant>,
    pub(crate) stale: u64,
}

impl Deposit {
    pub(crate) fn new(gen: u64, size: (usize, usize), identity: Option<Identity>, used: [bool; NCH]) -> Self {
        Self {
            gen,
            size,
            identity,
            records: 0,
            triggered: 0,
            planes: DiffPlane::bands(size, used),
            folds: Vec::new(),
            edges: [0; 4],
            off: (0.0, 0.0),
            newest: None,
            read_at: None,
            stale: 0,
        }
    }

    pub(crate) fn matches(&self, gen: u64, w: usize, h: usize) -> bool {
        self.gen == gen && self.size == (w, h)
    }

    pub(crate) fn size(&self) -> (usize, usize) {
        self.size
    }

    pub(crate) fn reset(&mut self, identity: Option<Identity>) {
        for band in &mut self.planes {
            band.clear();
        }
        self.identity = identity;
        self.records = 0;
        self.triggered = 0;
        self.folds.clear();
        self.edges = [0; 4];
        self.off = (0.0, 0.0);
        self.newest = None;
        self.read_at = None;
        self.stale = 0;
    }

    pub(crate) fn reject(&self, identity: Option<Identity>, counters: &Counters) {
        if !crate::drawpipe::feed::same_pair(self.identity, identity) {
            Counters::add(&counters.losses.pipe, self.records);
        }
        Counters::add(&counters.losses.folds, self.folds.iter().map(|(_, f)| f.n).sum());
        Counters::add(&counters.dropped, self.records);
    }
}

#[derive(Default)]
pub(crate) struct Slot {
    closed: bool,
    pending: Option<Deposit>,
    spare: Vec<Deposit>,
}

impl Slot {
    pub(crate) fn put(&mut self, deposit: Deposit) -> Option<Deposit> {
        if self.pending.is_some() {
            return Some(deposit);
        }
        self.pending = Some(deposit);
        crate::pacing::ready();
        None
    }
}

/// The condition is used only at the u16 weight limit; ordinary hand-offs never wait on the UI.
#[derive(Default)]
pub(crate) struct Handoff {
    state: std::sync::Mutex<Slot>,
    space: std::sync::Condvar,
}

impl Handoff {
    pub(crate) fn take(&self) -> Option<Deposit> {
        let deposit = oos_hal::sync::lock(&self.state).pending.take();
        self.space.notify_all();
        deposit
    }

    pub(crate) fn put(&self, deposit: Deposit) -> Option<Deposit> {
        oos_hal::sync::lock(&self.state).put(deposit)
    }
    pub(crate) fn recycle(&self, deposit: Deposit) {
        oos_hal::sync::lock(&self.state).spare.push(deposit);
    }
    pub(crate) fn spare(&self) -> Option<Deposit> {
        oos_hal::sync::lock(&self.state).spare.pop()
    }

    pub(crate) fn send(&self, deposit: Deposit) -> Option<Deposit> {
        let mut state = oos_hal::sync::lock(&self.state);
        while state.pending.is_some() && !state.closed {
            state = self.space.wait(state).unwrap_or_else(std::sync::PoisonError::into_inner);
        }
        if state.closed {
            return Some(deposit);
        }
        state.put(deposit)
    }

    pub(crate) fn close(&self) {
        oos_hal::sync::lock(&self.state).closed = true;
        self.space.notify_all();
    }
}
