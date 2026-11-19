// SPDX-License-Identifier: GPL-2.0-or-later

pub(crate) mod count;
pub(crate) mod kernel;
mod planes;
pub(crate) mod pool;
pub(crate) mod standard;
mod tap;
pub(crate) use planes::DiffBand;
pub(crate) use tap::{TapOrder, TapRead, TapReservation};
#[cfg(test)]
mod tests;

use crate::drawpipe::Counters;
use crate::model::NCH;
use oos_core::change::Identity;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub(crate) const HIT_LIMIT: u64 = u16::MAX as u64 / 24;

#[derive(Default)]
pub(crate) struct Extent {
    oldest: Option<Instant>,
    newest: Option<Instant>,
    hits: u64,
}

impl Extent {
    pub(crate) fn include(&mut self, at: Instant) {
        self.oldest = Some(self.oldest.map_or(at, |oldest| oldest.min(at)));
        self.newest = Some(self.newest.map_or(at, |newest| newest.max(at)));
    }

    pub(crate) fn newest(&self) -> Option<Instant> {
        self.newest
    }

    pub(crate) fn expired(&self, now: Instant) -> bool {
        self.oldest.is_some_and(|oldest| now.saturating_duration_since(oldest) > Duration::from_millis(67))
    }

    pub(crate) fn add_hits(&mut self, hits: u64) {
        self.hits = self.hits.saturating_add(hits);
    }

    pub(crate) fn hits(&self) -> u64 {
        self.hits
    }
}

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
    pub(crate) extent: Extent,
    pub(crate) progress: crate::claim_timing::Progress,
    pub(crate) taps: std::ops::Range<u64>,
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
            extent: Extent::default(),
            progress: Default::default(),
            taps: 0..0,
        }
    }

    pub(crate) fn matches(&self, gen: u64, w: usize, h: usize) -> bool {
        self.gen == gen && self.size == (w, h)
    }

    pub(crate) fn format(&self) -> count::CountFormat {
        self.planes.first().map_or(count::CountFormat::Narrow, |band| band.format())
    }

    pub(crate) fn set_format(&mut self, format: count::CountFormat) {
        for band in &mut self.planes {
            band.set_format(format);
        }
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
        self.extent = Extent::default();
        self.progress = Default::default();
        self.taps = 0..0;
    }

    pub(crate) fn include_tap(&mut self, number: u64) {
        if self.taps.is_empty() {
            self.taps.start = number;
        }
        self.taps.end = number + 1;
    }

    pub(crate) fn reject(&self, identity: Option<Identity>, counters: &Counters) {
        counters.reject_deposit(self.identity, identity, self.records);
        self.discard_folds(counters);
    }

    pub(crate) fn discard_folds(&self, counters: &Counters) {
        Counters::add(&counters.losses.folds, self.folds.iter().map(|(_, f)| f.n).sum());
    }
}

#[derive(Clone, Copy)]
pub(crate) struct TapView {
    worker_gen: u64,
    mapping_gen: u64,
    size: (usize, usize),
}

impl TapView {
    pub(crate) fn new(worker_gen: u64, mapping_gen: u64, size: (usize, usize)) -> Self {
        Self { worker_gen, mapping_gen, size }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum TapProjection {
    Unpainted,
    Projected { gen: u64, size: (usize, usize) },
    Accounted { mapping_gen: u64, size: (usize, usize) },
    Expired,
}

pub(crate) struct Tap {
    pub(crate) frame: Arc<crate::board::Frame>,
    pub(crate) identity: Option<Identity>,
    pub(crate) projection: TapProjection,
}

impl Tap {
    fn samples(&self) -> usize {
        self.frame.ch.iter().map(Vec::capacity).sum()
    }
}

#[derive(Default)]
struct Slot {
    closed: bool,
    claimed: bool,
    #[cfg(test)]
    waiting: bool,
    reset_before: u64,
    pending: Option<Deposit>,
    spare: Vec<Deposit>,
    taps: tap::Queue,
}

impl Slot {
    fn occupied(&self) -> bool {
        self.claimed || self.pending.is_some()
    }

    #[expect(clippy::result_large_err, reason = "Return the rejected deposit without allocating a box")]
    fn put(&mut self, deposit: Deposit) -> Result<(), Deposit> {
        if self.occupied() || deposit.predates(self.reset_before) {
            return Err(deposit);
        }
        self.pending = Some(deposit);
        Ok(())
    }
}

pub(crate) struct Claim<'a> {
    pub(crate) deposit: Option<Deposit>,
    slot: &'a Handoff,
}

impl Drop for Claim<'_> {
    fn drop(&mut self) {
        oos_hal::sync::lock(&self.slot.state).claimed = false;
        self.slot.space.notify_all();
    }
}

/// The picture slot and waveform FIFO have independent capacity. Reset and close release both waits.
#[derive(Default)]
pub(crate) struct Handoff {
    state: std::sync::Mutex<Slot>,
    space: std::sync::Condvar,
}

impl Handoff {
    pub(crate) fn claim(&self) -> Option<Claim<'_>> {
        let mut state = oos_hal::sync::lock(&self.state);
        let deposit = state.pending.take()?;
        state.claimed = true;
        Some(Claim { deposit: Some(deposit), slot: self })
    }

    #[cfg(test)]
    pub(crate) fn take(&self) -> Option<Deposit> {
        self.claim()?.deposit.take()
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
        while state.occupied() && !state.closed && !deposit.predates(state.reset_before) {
            #[cfg(test)]
            {
                state.waiting = true;
                self.space.notify_all();
            }
            state = self.space.wait(state).unwrap_or_else(std::sync::PoisonError::into_inner);
        }
        #[cfg(test)]
        {
            state.waiting = false;
        }
        if state.closed {
            return Err(deposit);
        }
        state.put(deposit).inspect(|()| crate::pacing::ready())
    }

    #[cfg(test)]
    pub(crate) fn wait_for_sender(&self) {
        let state = oos_hal::sync::lock(&self.state);
        let (state, _) =
            self.space.wait_timeout_while(state, crate::ui::lifecycle::test_wait::HANG_GUARD, |s| !s.waiting).unwrap();
        assert!(state.waiting, "the capacity handoff must wait while the UI slot is occupied");
    }

    pub(crate) fn reset(&self, reset_before: u64) -> Option<Deposit> {
        let mut state = oos_hal::sync::lock(&self.state);
        state.reset_before = reset_before;
        state.spare = Vec::new();
        let taps = state.taps.clear();
        let pending = state.pending.take();
        drop(state);
        drop(taps);
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
        let mut state = oos_hal::sync::lock(&self.state);
        state.closed = true;
        let taps = state.taps.clear();
        drop(state);
        drop(taps);
        self.space.notify_all();
    }
}
