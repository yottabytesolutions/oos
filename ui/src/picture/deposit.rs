// SPDX-License-Identifier: GPL-2.0-or-later

pub(crate) mod kernel;
mod planes;
pub(crate) mod pool;
pub(crate) mod standard;
pub(crate) use planes::DiffBand;
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
    pub(crate) progress: crate::claim_timing::Progress,
}

impl Deposit {
    pub(crate) fn new(gen: u64, size: (usize, usize), identity: Option<Identity>, used: [bool; NCH]) -> Self {
        Self {
            gen,
            size,
            identity,
            records: 0,
            triggered: 0,
            planes: DiffBand::bands(size, used),
            folds: Vec::new(),
            edges: [0; 4],
            off: (0.0, 0.0),
            newest: None,
            read_at: None,
            progress: Default::default(),
        }
    }

    pub(crate) fn matches(&self, gen: u64, w: usize, h: usize) -> bool {
        self.gen == gen && self.size == (w, h)
    }

    pub(crate) fn size(&self) -> (usize, usize) {
        self.size
    }

    pub(crate) fn predates(&self, view_gen: u64) -> bool {
        self.gen < view_gen
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
        self.progress = Default::default();
    }

    pub(crate) fn reject(&self, identity: Option<Identity>, counters: &Counters) {
        counters.reject_deposit(self.identity, identity, self.records);
        Counters::add(&counters.losses.folds, self.folds.iter().map(|(_, f)| f.n).sum());
    }
}

#[derive(Default)]
struct Slot {
    closed: bool,
    reset_before: u64,
    pending: Option<Deposit>,
    spare: Vec<Deposit>,
}

impl Slot {
    #[expect(clippy::result_large_err, reason = "Return the rejected deposit without allocating a box")]
    fn put(&mut self, deposit: Deposit) -> Result<(), Deposit> {
        if self.pending.is_some() || deposit.predates(self.reset_before) {
            return Err(deposit);
        }
        self.pending = Some(deposit);
        Ok(())
    }
}

/// Only the u16 weight limit waits for space. Reset and close also release that wait.
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

    #[expect(clippy::result_large_err, reason = "Return the rejected deposit without allocating a box")]
    pub(crate) fn put(&self, deposit: Deposit) -> Result<(), Deposit> {
        oos_hal::sync::lock(&self.state).put(deposit).inspect(|()| crate::pacing::ready())
    }
    pub(crate) fn recycle(&self, deposit: Deposit) {
        let mut state = oos_hal::sync::lock(&self.state);
        if !deposit.predates(state.reset_before) {
            state.spare.push(deposit);
        }
    }
    pub(crate) fn spare(&self) -> Option<Deposit> {
        oos_hal::sync::lock(&self.state).spare.pop()
    }

    #[expect(clippy::result_large_err, reason = "Return the rejected deposit without allocating a box")]
    pub(crate) fn send(&self, deposit: Deposit) -> Result<(), Deposit> {
        let mut state = oos_hal::sync::lock(&self.state);
        while state.pending.is_some() && !state.closed && !deposit.predates(state.reset_before) {
            state = self.space.wait(state).unwrap_or_else(std::sync::PoisonError::into_inner);
        }
        if state.closed {
            return Err(deposit);
        }
        state.put(deposit).inspect(|()| crate::pacing::ready())
    }

    pub(crate) fn reset(&self, reset_before: u64) -> Option<Deposit> {
        let mut state = oos_hal::sync::lock(&self.state);
        state.reset_before = reset_before;
        state.spare = Vec::new();
        let pending = state.pending.take();
        self.space.notify_all();
        pending
    }

    pub(crate) fn reset_before(&self) -> u64 {
        oos_hal::sync::lock(&self.state).reset_before
    }

    pub(crate) fn closed(&self) -> bool {
        oos_hal::sync::lock(&self.state).closed
    }

    pub(crate) fn close(&self) {
        oos_hal::sync::lock(&self.state).closed = true;
        self.space.notify_all();
    }
}
