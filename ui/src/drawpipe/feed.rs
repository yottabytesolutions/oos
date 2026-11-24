// SPDX-License-Identifier: GPL-2.0-or-later

use super::{clock::StageClock, *};
use crate::{board::Frame, model::*, sparse::Interp};
use oos_core::change::Identity;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::{ops::Range, time::Instant};

mod placement;
mod rejected;
mod view;
pub(super) use placement::{input as placement_input, words as placement_words};
pub(super) use view::{batch_view, prepare_view, set_view, stage_identity, stage_view};
pub(crate) use view::{same_pair, PreparedView};

pub(super) struct Header {
    pair: (u64, u64),
    identity: Option<Identity>,
    pub(super) tap: Option<Arc<TapReservation>>,
    pub(super) flight: Mutex<Option<admission::Flight>>,
    pub(super) order: Option<crate::picture::deposit::TapOrder>,
    pub(super) newest_from_dma: bool,
    pub(super) reader: Option<(Arc<View>, Option<Identity>)>,
    #[cfg(any(target_os = "linux", test))]
    pub(super) trace: Option<Box<route::RouteTrace>>,
}
impl Header {
    fn new(stamp: u64, identity: Option<Identity>) -> Self {
        let cfg = identity.map_or(0, |id| id.gen());
        #[cfg(test)]
        let cfg = if identity.is_none() { crate::pacing::APPLIED.load(Ordering::Relaxed) } else { cfg };
        Self {
            pair: (cfg, stamp),
            identity,
            tap: None,
            order: None,
            flight: Mutex::default(),
            newest_from_dma: false,
            reader: None,
            #[cfg(any(target_os = "linux", test))]
            trace: None,
        }
    }
    #[cfg(any(target_os = "linux", test))]
    fn attach_trace(&mut self, mut raw: Box<dyn RawBatch>, range: Range<usize>) -> Box<dyn RawBatch> {
        // Erase the outer Box before dispatching. Test adapters implement RawBatch for the Box as well.
        self.trace = raw.as_mut().take_route_trace().map(|mut trace| {
            trace.submitted(range);
            trace
        });
        raw
    }

    pub(super) fn identity(&self) -> Option<Identity> {
        self.identity
    }
    #[cfg(test)]
    pub(super) fn stamp(&self) -> u64 {
        self.pair.1
    }
    pub(super) fn tag(&self, f: &mut Frame) {
        (f.cfg, f.stamp) = self.pair;
    }
    pub(super) fn cfg(&self) -> u64 {
        self.pair.0
    }
}

impl Pipe {
    pub(crate) fn fund_taps(
        &self,
        share: crate::picture::deposit::admission::Share,
        interval: std::time::Duration,
    ) -> Option<()> {
        self.slot.fund_taps(share, interval)
    }
    pub(crate) fn staged_taps(&self) -> bool {
        self.slot.staged_taps()
    }
    pub(crate) fn has_funded_taps(&self, id: Identity) -> bool {
        self.slot.has_funded_taps(id)
    }
    pub(crate) fn retire_tap_plan(&self) {
        self.slot.retire_tap_plan();
    }
    pub(crate) fn stage_tap_hold(&self, hold: bool) {
        self.slot.stage_tap_hold(hold);
    }
    #[cfg(any(target_os = "linux", test))]
    pub(crate) fn has_tap_plan(&self, id: Identity, format: [u64; 6]) -> bool {
        self.slot.has_tap_plan(id, format)
    }
    #[cfg(any(target_os = "linux", test))]
    pub(crate) fn admit_taps(
        &self,
        id: Identity,
        format: [u64; 6],
        samples: usize,
        limit: std::time::Duration,
    ) -> bool {
        self.slot.admit_taps(id, format, samples, limit)
    }
    pub(crate) fn tap_drain(&self) -> usize {
        self.slot.tap_drain()
    }
    #[cfg(test)]
    pub(crate) fn reserve_plan(&self, id: Identity) -> Option<Arc<TapReservation>> {
        let submitted = oos_hal::sync::lock(&self.view).gen;
        self.slot.reserve_plan(submitted, id)
    }
    #[cfg(any(target_os = "linux", test))]
    pub(crate) fn reserve_plan_until(&self, id: Identity, canceled: &dyn Fn() -> bool) -> Option<Arc<TapReservation>> {
        let submitted = oos_hal::sync::lock(&self.view).gen;
        self.slot.reserve_plan_until(submitted, id, canceled)
    }

    #[cfg(any(target_os = "linux", test))]
    pub(crate) fn try_prearm_tap(&self, id: Identity) -> Option<Arc<TapReservation>> {
        let submitted = oos_hal::sync::lock(&self.view).gen;
        let reservation = self.slot.try_reserve_plan(submitted, id)?;
        self.slot.try_start_cycle(&reservation, Instant::now())?;
        Some(reservation)
    }
    #[cfg(any(target_os = "linux", test))]
    pub(crate) fn current_tap(&self, reservation: &TapReservation) -> bool {
        self.slot.current_reservation(reservation)
    }
    #[cfg(any(target_os = "linux", test))]
    pub(crate) fn wait_for_tap_cycle_until(
        &self,
        reservation: &TapReservation,
        canceled: &dyn Fn() -> bool,
    ) -> Option<()> {
        self.slot.wait_for_cycle_until(reservation, Instant::now, canceled)
    }

    #[cfg(test)]
    pub(crate) fn tap_start(&self, id: Identity, now: Instant) -> Option<std::time::Duration> {
        self.slot.tap_start(id, now)
    }

    #[cfg(test)]
    pub(crate) fn reserve_waveform(&self, frame: &Frame, identity: Identity) -> Option<Arc<TapReservation>> {
        self.reserve_waveform_until(frame, identity, &|| false)
    }

    #[cfg(any(target_os = "linux", test))]
    pub(crate) fn reserve_waveform_until(
        &self,
        frame: &Frame,
        identity: Identity,
        canceled: &dyn Fn() -> bool,
    ) -> Option<Arc<TapReservation>> {
        let submitted = oos_hal::sync::lock(&self.view).gen;
        self.slot.reserve_tap_until(frame.ch.iter().map(Vec::capacity).sum(), submitted, Some(identity), canceled)
    }

    #[cfg(any(target_os = "linux", test))]
    pub(crate) fn grant(&self, identity: Identity, bound: u32, budget: std::time::Duration) -> super::admission::Grant {
        self.admission.grant(identity, bound, budget)
    }

    #[cfg(any(target_os = "linux", test))]
    pub(crate) fn in_flight(&self) -> bool {
        self.admission.busy()
    }

    #[cfg(any(target_os = "linux", test))]
    pub(crate) fn read_limit(&self, identity: Identity, live: Option<(u64, u32)>) -> Option<u32> {
        let state = oos_hal::sync::lock(&self.view);
        let view = state.view.as_ref().filter(|_| same_pair(state.identity, Some(identity)))?;
        // An unclaimed batch may acquire any same-identity mapping. Bound the whole reachable
        // envelope before arm; the worker still verifies its immutable claim before writing tiles.
        use crate::picture::deposit::{count::CountFormat, standard};
        let columns = view.w.min(crate::ui::batch_min_columns()).max(1);
        let window = view.m.time_per_div() as f64 * HDIV as f64;
        let fallback = view.record_hit_bound_at(columns);
        let zoom_hits = view.zoom.map_or(1, |zoom| view.zoom_hit_bound(zoom.size().0.min(columns).max(1)));
        let hits = |rate: u64, k: u32, dots: bool| {
            let spacing = k as f64 / rate as f64 / window;
            // Screen::of_size caps layout width at 7680. Wider projections can select the
            // legacy sparse record even when the smallest projection uses dense placement.
            let sparse = if spacing * 7680.0 > 0.5 { fallback } else { 1 };
            let dense = if dots { standard::dot_hit_bound(spacing * columns as f64) } else { 1 };
            sparse.max(dense)
        };
        let (per_record, newest_extra) = live.map_or_else(
            || (fallback, 0),
            |(rate, k)| {
                // Explicit Dots disables compression and changes effective read format/gen.
                // Pair reads can remap geometry, but cannot become raw Dots under this identity.
                let per_record = hits(rate, k, k == 1);
                let newest = hits(rate, 1, k == 1);
                (per_record, newest.saturating_sub(per_record))
            },
        );
        let per_record = per_record.max(zoom_hits);
        let limit = CountFormat::for_hits(per_record.saturating_add(newest_extra)).hit_limit();
        let records = limit.saturating_sub(newest_extra) / per_record;
        // Same-identity Lines must also fit their unchanged narrow format, including sparse fallback.
        let records = if limit > crate::picture::deposit::HIT_LIMIT {
            records.min(crate::picture::deposit::HIT_LIMIT / fallback)
        } else {
            records
        };
        Some(records.max(1) as u32)
    }

    /// `feed_codes` without the board's view of the read-out words (tests).
    #[cfg(test)]
    pub fn feed(&self, raw: Box<dyn RawBatch>, fb: usize, frames: usize, stamp: u64, decode: DecodeInto) {
        self.feed_batch(raw, fb, 0..frames, (stamp, None), decode, None);
    }

    /// Submit the first `frames` frames with an optional direct-code layout for host tests.
    #[cfg(test)]
    pub fn feed_codes(
        &self,
        raw: Box<dyn RawBatch>,
        fb: usize,
        frames: usize,
        stamp: u64,
        decode: DecodeInto,
        codes: Option<CodesInto>,
    ) {
        self.feed_batch(raw, fb, 0..frames, (stamp, None), decode, codes);
    }

    /// Whole kept range and its newest waveform, already decoded for record readers.
    /// Returns the waveform for the record path if the coordinator cannot queue the batch.
    #[allow(clippy::too_many_arguments)] // Buffer geometry, identity and conversion describe one ring read.
    pub fn feed_with(
        &self,
        raw: Box<dyn RawBatch>,
        fb: usize,
        frames: Range<usize>,
        identity: Identity,
        decode: DecodeInto,
        codes: Option<CodesInto>,
        newest: Option<Arc<Frame>>,
    ) -> Option<Arc<Frame>> {
        self.feed_at(raw, fb, frames, (identity, Instant::now()), decode, codes, newest)
    }

    #[allow(clippy::too_many_arguments)] // Buffer geometry, timestamp and conversion describe one ring read.
    pub(crate) fn feed_at(
        &self,
        raw: Box<dyn RawBatch>,
        fb: usize,
        frames: Range<usize>,
        (identity, fed): (Identity, Instant),
        decode: DecodeInto,
        codes: Option<CodesInto>,
        newest: Option<Arc<Frame>>,
    ) -> Option<Arc<Frame>> {
        self.submit(
            raw,
            fb,
            frames,
            (Header::new(identity.epoch(), Some(identity)), fed.into()),
            decode.into(),
            codes,
            newest,
        )
        .1
    }

    #[cfg(test)]
    pub fn feed_prepared(
        &self,
        raw: Box<dyn RawBatch>,
        fb: usize,
        frames: Range<usize>,
        (identity, fed): (Identity, Instant),
        decode: IndexedDecodeInto,
        newest: Option<Arc<Frame>>,
    ) -> Option<Arc<Frame>> {
        self.feed_readout((raw, fb, frames), (identity, fed), decode, None, (newest, None))
    }

    #[cfg(test)]
    pub(crate) fn feed_readout(
        &self,
        (raw, fb, frames): (Box<dyn RawBatch>, usize, Range<usize>),
        (identity, clock): (Identity, impl Into<StageClock>),
        decode: IndexedDecodeInto,
        codes: Option<CodesInto>,
        (newest, tap): (Option<Arc<Frame>>, Option<Arc<TapReservation>>),
    ) -> Option<Arc<Frame>> {
        self.feed_source((raw, fb, frames), (identity, clock), (decode, codes), (newest, tap), false)
    }

    #[cfg(any(target_os = "linux", test))]
    pub(crate) fn feed_source(
        &self,
        (raw, fb, frames): (Box<dyn RawBatch>, usize, Range<usize>),
        (identity, clock): (Identity, impl Into<StageClock>),
        (decode, codes): (IndexedDecodeInto, Option<CodesInto>),
        (newest, tap): (Option<Arc<Frame>>, Option<Arc<TapReservation>>),
        newest_from_dma: bool,
    ) -> Option<Arc<Frame>> {
        let mut header = Header::new(identity.epoch(), Some(identity));
        header.tap = tap;
        header.newest_from_dma = newest_from_dma;
        self.submit(raw, fb, frames, (header, clock.into()), decoder::Decoder::Prepared(decode), codes, newest).1
    }

    #[cfg(test)]
    pub(super) fn feed_batch(
        &self,
        raw: Box<dyn RawBatch>,
        fb: usize,
        frames: Range<usize>,
        header: (u64, Option<Identity>),
        decode: impl Into<decoder::Decoder>,
        codes: Option<CodesInto>,
    ) -> Arc<Batch> {
        self.submit(
            raw,
            fb,
            frames,
            (Header::new(header.0, header.1), Instant::now().into()),
            decode.into(),
            codes,
            None,
        )
        .0
    }

    #[allow(clippy::too_many_arguments)] // All arguments are immutable batch inputs.
    fn submit(
        &self,
        raw: Box<dyn RawBatch>,
        fb: usize,
        frames: Range<usize>,
        (mut header, clock): (Header, StageClock),
        decode: decoder::Decoder,
        codes: Option<CodesInto>,
        newest: Option<Arc<Frame>>,
    ) -> (Arc<Batch>, Option<Arc<Frame>>) {
        let first = frames.start;
        let frames = frames.end.min(raw.bytes().len().checked_div(fb).unwrap_or(0));
        #[cfg(any(target_os = "linux", test))]
        let raw = header.attach_trace(raw, first.min(frames)..frames);
        let state = oos_hal::sync::lock(&self.view);
        let seq = newest.as_ref().map(|frame| frame.seq);
        if seq.is_some() && same_pair(state.identity, header.identity()) {
            header.reader = state.view.as_ref().map(|view| (view.clone(), state.identity));
        }
        let view_gen_at_submit = header.tap.as_ref().map_or(state.gen, |tap| tap.submitted());
        drop(state);
        // The acquisition thread can publish a newer fallback as soon as submit returns.
        header.order = seq.and_then(|seq| self.slot.track_tap(view_gen_at_submit, header.identity(), seq));

        if clock.admitted() {
            let identity = header.identity().expect("hardware submission has an acquisition identity");
            *header.flight.get_mut().unwrap_or_else(std::sync::PoisonError::into_inner) =
                Some(self.reserve_flight(identity));
        }
        let mut b = Arc::new(Batch {
            lease: Lease { raw: Some(raw), home: self.home.clone(), source: self.source.clone() },
            fb,
            frames,
            decode,
            codes,
            header,
            view_gen_at_submit,
            next: AtomicUsize::new(first.min(frames)),
            counters: self.counters,
            live: self.live.clone(),
            newest,
            clock,
        });
        self.live.fetch_add(1, Ordering::Acquire);
        if self.enqueue(&mut b) {
            return (b, None);
        }
        let batch = Arc::get_mut(&mut b).expect("rejected intake did not share the batch");
        let fallback = batch.rejected_waveform();
        (b, fallback)
    }

    fn reserve_flight(&self, identity: Identity) -> admission::Flight {
        let queued = oos_hal::sync::lock(&self.workers).queued.load(Ordering::Acquire);
        if queued != 0 {
            Counters::add(&self.counters.losses.unexpected_full, 1);
            debug_assert_eq!(queued, 0, "admission allows only one batch in flight");
        }
        self.admission.submit(&self.source, identity, self.counters)
    }

    fn enqueue(&self, batch: &mut Arc<Batch>) -> bool {
        let (sender, queued) = {
            let workers = oos_hal::sync::lock(&self.workers);
            (workers.sender.clone(), workers.queued.clone())
        };
        let Some(tx) = sender else { return false };
        Arc::get_mut(batch).expect("the batch has not been submitted").clock.submit();
        queued.fetch_add(1, Ordering::AcqRel);
        let sent = tx.send(Message::Batch(batch.clone())).is_ok();
        if !sent {
            queued.fetch_sub(1, Ordering::AcqRel);
        }
        sent
    }
}
