// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use std::time::Instant;

pub(super) struct Header {
    pair: (u64, u64),
    identity: Option<oos_core::change::Identity>,
    pub(super) tap: Option<Arc<TapReservation>>,
    pub(super) order: Option<crate::picture::deposit::TapOrder>,
    pub(super) reader: Option<(Arc<View>, Option<oos_core::change::Identity>)>,
}
impl Header {
    fn new(stamp: u64, identity: Option<oos_core::change::Identity>) -> Self {
        let cfg = identity.map_or(0, |id| id.gen());
        #[cfg(test)]
        let cfg = if identity.is_none() { crate::pacing::APPLIED.load(Ordering::Relaxed) } else { cfg };
        Self { pair: (cfg, stamp), identity, tap: None, order: None, reader: None }
    }
    pub(super) fn identity(&self) -> Option<oos_core::change::Identity> {
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
    pub(crate) fn has_funded_taps(&self, id: oos_core::change::Identity) -> bool {
        self.slot.has_funded_taps(id)
    }
    pub(crate) fn retire_tap_plan(&self) {
        self.slot.retire_tap_plan();
    }
    pub(crate) fn hold_taps(&self, hold: bool) {
        self.slot.hold_taps(hold);
    }
    #[cfg(any(target_os = "linux", test))]
    pub(crate) fn has_tap_plan(&self, id: oos_core::change::Identity, format: [u64; 6]) -> bool {
        self.slot.has_tap_plan(id, format)
    }
    #[cfg(any(target_os = "linux", test))]
    pub(crate) fn admit_taps(
        &self,
        id: oos_core::change::Identity,
        format: [u64; 6],
        samples: usize,
        limit: std::time::Duration,
    ) -> bool {
        self.slot.admit_taps(id, format, samples, limit)
    }
    pub(crate) fn tap_drain(&self) -> usize {
        self.slot.tap_drain()
    }
    #[cfg(any(target_os = "linux", test))]
    pub(crate) fn reserve_plan(&self, id: oos_core::change::Identity) -> Option<Arc<TapReservation>> {
        let submitted = oos_hal::sync::lock(&self.view).gen;
        self.slot.reserve_plan(submitted, id)
    }
    #[cfg(any(target_os = "linux", test))]
    pub(crate) fn wait_for_tap_cycle(&self, reservation: &TapReservation) -> Option<()> {
        self.slot.wait_for_cycle(reservation, Instant::now)
    }
    #[cfg(test)]
    pub(crate) fn tap_start(&self, id: oos_core::change::Identity, now: Instant) -> Option<std::time::Duration> {
        self.slot.tap_start(id, now)
    }

    #[cfg(any(target_os = "linux", test))]
    pub(crate) fn reserve_waveform(
        &self,
        frame: &Frame,
        identity: oos_core::change::Identity,
    ) -> Option<Arc<TapReservation>> {
        let submitted = oos_hal::sync::lock(&self.view).gen;
        self.slot.reserve_tap(frame.ch.iter().map(Vec::capacity).sum(), submitted, Some(identity))
    }

    #[cfg(any(target_os = "linux", test))]
    pub(crate) fn read_limit(&self, identity: oos_core::change::Identity, live: Option<(u64, u32)>) -> Option<u32> {
        let state = oos_hal::sync::lock(&self.view);
        let view = state.view.as_ref().filter(|_| same_pair(state.identity, Some(identity)))?;
        // An unclaimed batch may acquire any same-identity mapping. Bound the whole reachable
        // envelope before arm; the worker still verifies its immutable claim before writing tiles.
        use crate::picture::deposit::{count::CountFormat, standard};
        let columns = view.w.min(crate::ui::batch_min_columns()).max(1);
        let window = view.m.time_per_div() as f64 * HDIV as f64;
        let fallback = standard::record_hit_bound(columns, crate::sparse::POINTS as f64, view.m.delay as f64 / window);
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
        frames: std::ops::Range<usize>,
        identity: oos_core::change::Identity,
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
        frames: std::ops::Range<usize>,
        (identity, fed): (oos_core::change::Identity, Instant),
        decode: DecodeInto,
        codes: Option<CodesInto>,
        newest: Option<Arc<Frame>>,
    ) -> Option<Arc<Frame>> {
        self.submit(raw, fb, frames, (Header::new(identity.epoch(), Some(identity)), fed), decode.into(), codes, newest)
            .1
    }

    #[cfg(test)]
    pub fn feed_prepared(
        &self,
        raw: Box<dyn RawBatch>,
        fb: usize,
        frames: std::ops::Range<usize>,
        (identity, fed): (oos_core::change::Identity, Instant),
        decode: IndexedDecodeInto,
        newest: Option<Arc<Frame>>,
    ) -> Option<Arc<Frame>> {
        self.feed_readout((raw, fb, frames), (identity, fed), decode, None, (newest, None))
    }

    #[cfg(any(target_os = "linux", test))]
    pub(crate) fn feed_readout(
        &self,
        (raw, fb, frames): (Box<dyn RawBatch>, usize, std::ops::Range<usize>),
        (identity, fed): (oos_core::change::Identity, Instant),
        decode: IndexedDecodeInto,
        codes: Option<CodesInto>,
        (newest, tap): (Option<Arc<Frame>>, Option<Arc<TapReservation>>),
    ) -> Option<Arc<Frame>> {
        let mut header = Header::new(identity.epoch(), Some(identity));
        header.tap = tap;
        self.submit(raw, fb, frames, (header, fed), decoder::Decoder::Prepared(decode), codes, newest).1
    }

    #[cfg(test)]
    pub(super) fn feed_batch(
        &self,
        raw: Box<dyn RawBatch>,
        fb: usize,
        frames: std::ops::Range<usize>,
        header: (u64, Option<oos_core::change::Identity>),
        decode: impl Into<decoder::Decoder>,
        codes: Option<CodesInto>,
    ) -> Arc<Batch> {
        self.submit(raw, fb, frames, (Header::new(header.0, header.1), Instant::now()), decode.into(), codes, None).0
    }

    #[allow(clippy::too_many_arguments)] // All arguments are immutable batch inputs.
    fn submit(
        &self,
        raw: Box<dyn RawBatch>,
        fb: usize,
        frames: std::ops::Range<usize>,
        (mut header, fed): (Header, Instant),
        decode: decoder::Decoder,
        codes: Option<CodesInto>,
        newest: Option<Arc<Frame>>,
    ) -> (Arc<Batch>, Option<Arc<Frame>>) {
        let first = frames.start;
        let frames = frames.end.min(raw.bytes().len().checked_div(fb).unwrap_or(0));
        let state = oos_hal::sync::lock(&self.view);
        if newest.is_some() && same_pair(state.identity, header.identity()) {
            header.reader = state.view.as_ref().map(|view| (view.clone(), state.identity));
        }
        let view_gen_at_submit = header.tap.as_ref().map_or(state.gen, |tap| tap.submitted());
        drop(state);
        // The acquisition thread can publish a newer fallback as soon as submit returns.
        header.order = newest.as_ref().and_then(|f| self.slot.track_tap(view_gen_at_submit, header.identity(), f.seq));
        let mut b = Arc::new(Batch {
            lease: Lease { raw: Some(raw), home: self.home.clone() },
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
            fed,
        });
        self.live.fetch_add(1, Ordering::Acquire);
        if let Some(tx) = &oos_hal::sync::lock(&self.workers).sender {
            if tx.try_send(Message::Batch(b.clone())).is_ok() {
                return (b, None);
            }
            eprintln!("oos-ui: deposit intake unavailable");
        }
        let batch = Arc::get_mut(&mut b).expect("rejected intake did not share the batch");
        let fallback = batch.newest.take();
        drop(batch.header.order.take());
        if let Some(charge) = &batch.header.tap {
            charge.fallback();
        }
        let next = batch.next.get_mut();
        // The record path accounts for the returned waveform.
        *next += usize::from(fallback.is_some() && *next < batch.frames);
        (b, fallback)
    }
}

pub(crate) fn same_pair(a: Option<oos_core::change::Identity>, b: Option<oos_core::change::Identity>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => (a.gen(), a.epoch()) == (b.gen(), b.epoch()),
        (None, None) => true,
        _ => false,
    }
}

pub(super) fn batch_view(b: &Batch, state: &ViewState, reset_before: u64) -> Option<Arc<View>> {
    let v = state
        .view
        .clone()
        .filter(|_| b.view_gen_at_submit >= reset_before && accepts(b, legacy_stamp(state), state.identity));
    if v.is_none() {
        let records = b.frames.saturating_sub(b.next.swap(b.frames, Ordering::Relaxed)) as u64;
        if !same_pair(b.header.identity(), state.identity) {
            Counters::add(&b.counters.losses.pipe, records);
        }
        b.counters.discard(Lost::Settings, records);
    }
    v
}

pub(super) fn accepts(b: &Batch, legacy_stamp: u64, identity: Option<oos_core::change::Identity>) -> bool {
    match (b.header.identity(), identity) {
        (Some(_), Some(_)) => same_pair(b.header.identity(), identity),
        #[cfg(test)]
        (None, None) => b.header.stamp() >= legacy_stamp,
        _ => {
            let _ = legacy_stamp;
            false
        }
    }
}

pub(super) fn legacy_stamp(state: &ViewState) -> u64 {
    #[cfg(test)]
    {
        state.stamp
    }
    #[cfg(not(test))]
    {
        let _ = state;
        0
    }
}

pub(super) fn set_view(
    pipe: &Pipe,
    m: Option<&Model>,
    w: usize,
    h: usize,
    interp: Interp,
    identity: Option<oos_core::change::Identity>,
) {
    let mut v = oos_hal::sync::lock(&pipe.view);
    let Some(m) = m else {
        (v.view, v.key) = (None, None);
        pipe.retire_tap_plan();
        return;
    };
    let layout = oos_core::change::Layout::new((w, h), (0, 0), (0, 0), None, 0, 0);
    let key = oos_core::change::WorkerViewKey::of(m, layout, &crate::model::config());
    let worker_changed = v.key.as_ref() != Some(&key);
    let statistics = identity.is_some_and(|id| v.identity.map(|old| old.stats_gen()) != Some(id.stats_gen()));
    if worker_changed {
        let mapping_key = oos_core::change::MappingKey::of(m, layout);
        v.mapping_gen += u64::from(v.mapping_key.as_ref() != Some(&mapping_key));
        v.mapping_key = Some(mapping_key);
    }
    if worker_changed || statistics {
        crate::pacing::VIEWS.fetch_add(1, Ordering::Relaxed);
        v.gen += u64::from(worker_changed);
        v.view = Some(Arc::new(View { gen: v.gen, m: m.clone(), interp, w, h }));
        v.key = Some(key);
    }
    #[cfg(test)]
    {
        v.stamp = m.hw_stamp;
    }
    if let Some(id) = identity {
        if !same_pair(v.identity, Some(id)) {
            pipe.slot.set_tap_identity(id);
        }
        v.identity = Some(id);
    }
}
