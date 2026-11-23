// SPDX-License-Identifier: GPL-2.0-or-later

pub(crate) mod admission;
mod backing;
pub(crate) mod count;
pub(crate) mod envelope;
pub(crate) mod interpolation;
pub(crate) mod kernel;
mod maps;
mod planes;
pub(crate) mod pool;
mod ready;
mod same_picture;
pub(crate) mod standard;
mod tap;
pub(crate) use planes::DiffBand;
pub(crate) use ready::Claim;
pub(crate) use tap::{TapOrder, TapRead, TapReservation, TapSettlement};
#[cfg(test)]
mod tests;

use crate::drawpipe::{clock::BatchClocks, Counters};
use crate::model::NCH;
use crate::picture::PerMap;
use oos_core::change::Identity;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub(crate) const HIT_LIMIT: u64 = u16::MAX as u64 / 24;

#[derive(Default)]
pub(crate) struct Extent {
    #[cfg(feature = "private-picture-counts")]
    eligible_oldest: Option<Instant>,

    newest: Option<Instant>,
    hits: u64,
    clocks: BatchClocks,
}

impl Extent {
    pub(crate) fn include(&mut self, at: Instant) {
        self.newest = Some(self.newest.map_or(at, |newest| newest.max(at)));
    }

    pub(crate) fn clocks(&mut self) -> &mut BatchClocks {
        &mut self.clocks
    }

    #[cfg(feature = "private-picture-counts")]
    pub(crate) fn eligible_oldest(&self) -> Option<Instant> {
        self.eligible_oldest
    }
    #[cfg(feature = "private-picture-counts")]
    pub(crate) fn contributed(&mut self, at: Instant, records: u64) {
        if records > 0 {
            self.eligible_oldest = Some(self.eligible_oldest.map_or(at, |oldest| oldest.min(at)));
        }
    }
    pub(crate) fn newest(&self) -> Option<Instant> {
        self.newest
    }

    pub(crate) fn add_hits(&mut self, hits: u64) {
        self.hits = self.hits.saturating_add(hits);
    }

    pub(crate) fn hits(&self) -> u64 {
        self.hits
    }
}

struct Maps {
    gen: u64,
    sizes: PerMap<(usize, usize)>,
    zoom: Option<Vec<DiffBand>>,
    weight: PerMap<f64>,
}

pub(crate) struct Deposit {
    maps: Maps,
    backing: Option<backing::Lease>,
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
    /// The newest input read represented by a nonempty publication.
    pub(crate) fn ready_origin(&self) -> Option<Instant> {
        (self.records > 0).then(|| self.extent.newest()).flatten()
    }

    #[cfg(test)]
    pub(crate) fn new(gen: u64, size: (usize, usize), identity: Option<Identity>, used: [bool; NCH]) -> Self {
        Self::new_picture(gen, PerMap::new(size, None), identity, used)
    }

    fn new_picture(gen: u64, sizes: PerMap<(usize, usize)>, identity: Option<Identity>, used: [bool; NCH]) -> Self {
        Self {
            maps: Maps::new(gen, sizes, used),
            backing: None,
            identity,
            records: 0,
            triggered: 0,
            planes: DiffBand::bands(sizes.main(), used),
            folds: Vec::new(),
            edges: [0; 4],
            off: (0.0, 0.0),
            extent: Extent::default(),
            progress: Default::default(),
            taps: 0..0,
        }
    }

    pub(crate) fn matches(&self, gen: u64, w: usize, h: usize) -> bool {
        self.maps.gen == gen && self.size() == (w, h)
    }

    pub(crate) fn format(&self) -> count::CountFormat {
        self.planes.first().map_or(count::CountFormat::Narrow, |band| band.format())
    }

    pub(crate) fn set_format(&mut self, format: count::CountFormat) {
        for band in self.all_bands_mut() {
            band.set_format(format);
        }
    }

    pub(crate) fn size(&self) -> (usize, usize) {
        self.maps.sizes.main()
    }

    pub(crate) fn predates(&self, view_gen: u64) -> bool {
        self.maps.gen < view_gen
    }

    pub(crate) fn reset(&mut self, identity: Option<Identity>) {
        for band in self.all_bands_mut() {
            band.clear();
        }
        self.maps.reset();
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
        self.reject_timing(counters, Instant::now());
        self.discard_folds(counters);
    }

    pub(crate) fn reject_timing(&self, counters: &Counters, now: Instant) {
        counters.metrics.finish(self.identity, &self.extent.clocks, now, false);
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

pub(crate) enum TapProjection {
    Unpainted,
    Average(crate::drawpipe::average::Output),
    Projected { gen: u64, size: (usize, usize) },
    Accounted { mapping_gen: u64, size: (usize, usize) },
    Recoverable,
    Expired,
}

pub(crate) struct Tap {
    pub(crate) frame: Arc<crate::board::Frame>,
    pub(crate) identity: Option<Identity>,
    pub(crate) projection: TapProjection,
    pub(crate) folds: Option<crate::framestats::TapFolds>,
}

impl Tap {
    fn samples(&self) -> usize {
        let raw = self.frame.ch.iter().map(Vec::capacity).sum::<usize>();
        raw + match &self.projection {
            TapProjection::Average(output) => output.capacity(),
            _ => 0,
        }
    }
}

#[derive(Default)]
struct Slot {
    commands: u64,
    closed: bool,
    claimed: bool,
    #[cfg(test)]
    waiting: bool,
    reset_before: u64,
    backings: usize,
    #[cfg(test)]
    backing_waiting: bool,
    pending: Option<Deposit>,
    spare: Vec<Deposit>,
    taps: tap::Queue,
}

impl Slot {
    fn accepts(&self, deposit: &Deposit) -> bool {
        !deposit.predates(self.reset_before) && self.taps.accepts_deposit(deposit.identity)
    }

    fn occupied(&self) -> bool {
        self.claimed || self.pending.is_some()
    }

    #[expect(clippy::result_large_err, reason = "Return the rejected deposit without allocating a box")]
    fn put(&mut self, mut deposit: Deposit) -> Result<(), Deposit> {
        if self.closed || self.occupied() || !self.accepts(&deposit) {
            return Err(deposit);
        }
        deposit.extent.clocks().published();
        self.pending = Some(deposit);
        Ok(())
    }
}

/// The picture slot and waveform FIFO have independent capacity. Reset and close release both waits.
#[derive(Default)]
pub(crate) struct Handoff {
    ready: ready::Signal,
    state: Arc<std::sync::Mutex<Slot>>,
    space: Arc<std::sync::Condvar>,
}

impl Handoff {
    #[cfg(test)]
    pub(crate) fn backing_count(&self) -> usize {
        oos_hal::sync::lock(&self.state).backings
    }
    #[cfg(test)]
    pub(crate) fn wait_for_backing(&self) {
        backing::wait_for_backing(self);
    }
    pub(crate) fn owner_enable(&self, active: bool, wake: Option<Arc<crate::system::intake::Wake>>) {
        if self.ready.enable(active, wake) && oos_hal::sync::lock(&self.state).pending.is_some() {
            self.ready.published();
        }
    }
    pub(crate) fn owner_enabled(&self) -> bool {
        self.ready.enabled.load(std::sync::atomic::Ordering::Acquire)
    }
    pub(crate) fn owner_rearm(&self) {
        self.ready.rearm();
    }
    #[cfg(test)]
    pub(crate) fn acquire(
        &self,
        gen: u64,
        size: (usize, usize),
        identity: Option<Identity>,
        used: [bool; NCH],
    ) -> Option<Deposit> {
        self.acquire_picture(gen, PerMap::new(size, None), identity, used)
    }

    pub(crate) fn acquire_picture(
        &self,
        gen: u64,
        sizes: PerMap<(usize, usize)>,
        identity: Option<Identity>,
        used: [bool; NCH],
    ) -> Option<Deposit> {
        backing::acquire(self, gen, sizes, identity, used)
    }

    pub(crate) fn claim(&self) -> Option<Claim<'_>> {
        self.take_pending(|deposit| deposit.extent.clocks().claimed(Instant::now()))
    }

    /// Free the producer slot while the render owner stages immutable inputs for the picture step.
    pub(crate) fn stage(&self) -> Option<Claim<'_>> {
        self.take_pending(|deposit| deposit.extent.clocks().staged())
    }

    fn take_pending(&self, stamp: impl FnOnce(&mut Deposit)) -> Option<Claim<'_>> {
        let mut state = oos_hal::sync::lock(&self.state);
        let mut deposit = state.pending.take()?;
        stamp(&mut deposit);
        state.claimed = true;
        Some(Claim::new(self, deposit))
    }

    #[cfg(test)]
    pub(crate) fn take(&self) -> Option<Deposit> {
        self.claim()?.deposit.take()
    }

    #[expect(clippy::result_large_err, reason = "Return the rejected deposit without allocating a box")]
    pub(crate) fn put(&self, deposit: Deposit) -> Result<(), Deposit> {
        let read = deposit.ready_origin();
        let result = oos_hal::sync::lock(&self.state).put(deposit).inspect(|()| {
            crate::pacing::ready(read, Instant::now());
        });
        if result.is_ok() {
            self.ready.published();
        }
        result
    }
    pub(crate) fn recycle(&self, deposit: Deposit) {
        let mut state = oos_hal::sync::lock(&self.state);
        let foreign = deposit.backing.as_ref().is_some_and(|lease| !lease.belongs_to(&self.state));
        let retired = if foreign || deposit.predates(state.reset_before) {
            Some(deposit)
        } else {
            state.spare.push(deposit);
            None
        };
        drop(state);
        drop(retired);
        self.space.notify_all();
    }
    #[cfg(test)]
    pub(crate) fn spare(&self) -> Option<Deposit> {
        oos_hal::sync::lock(&self.state).spare.pop()
    }

    #[expect(clippy::result_large_err, reason = "Return the rejected deposit without allocating a box")]
    pub(crate) fn send(&self, deposit: Deposit) -> Result<(), Deposit> {
        self.send_timed(deposit).map(|_| ())
    }

    /// Returns the time spent placing the deposit into the slot once space is available.
    #[expect(clippy::result_large_err, reason = "Return the rejected deposit without allocating a box")]
    pub(crate) fn send_timed(&self, deposit: Deposit) -> Result<Duration, Deposit> {
        self.send_timed_with(deposit, Instant::now, crate::pacing::ready)
    }

    #[expect(clippy::result_large_err, reason = "Return the rejected deposit without allocating a box")]
    fn send_timed_with(
        &self,
        deposit: Deposit,
        now: impl Fn() -> Instant,
        ready: impl FnOnce(Option<Instant>, Instant),
    ) -> Result<Duration, Deposit> {
        let mut state = oos_hal::sync::lock(&self.state);
        while state.occupied() && !state.closed && state.accepts(&deposit) {
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
        let publishing = now();
        let read = deposit.ready_origin();
        let result = state.put(deposit).map(|()| now());

        drop(state);
        result.map(|published| {
            ready(read, published);
            self.ready.published();
            published.saturating_duration_since(publishing)
        })
    }

    #[cfg(test)]
    pub(crate) fn wait_for_sender(&self) {
        let state = oos_hal::sync::lock(&self.state);
        let (state, _) =
            self.space.wait_timeout_while(state, crate::ui::lifecycle::test_wait::HANG_GUARD, |s| !s.waiting).unwrap();
        assert!(state.waiting, "the capacity handoff must wait while the UI slot is occupied");
    }

    pub(crate) fn reset(&self, reset_before: u64) -> Option<Deposit> {
        self.ready.rearm();
        let mut state = oos_hal::sync::lock(&self.state);
        state.reset_before = reset_before;
        let spare = std::mem::take(&mut state.spare);
        let mut taps = state.taps.clear();
        let pending = state.pending.take();
        drop(state);
        drop(spare);
        taps.clear();
        oos_hal::sync::lock(&self.state).taps.restore_storage(taps);
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
