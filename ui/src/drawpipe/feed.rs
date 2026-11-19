// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

pub(super) struct Header {
    pair: (u64, u64),
    identity: Option<oos_core::change::Identity>,
}
impl Header {
    fn new(stamp: u64, identity: Option<oos_core::change::Identity>) -> Self {
        let cfg = identity.map_or(0, |id| id.gen());
        #[cfg(test)]
        let cfg = if identity.is_none() { crate::pacing::APPLIED.load(Ordering::Relaxed) } else { cfg };
        Self { pair: (cfg, stamp), identity }
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
    /// `feed_codes` without the board's view of the read-out words (tests).
    #[cfg(test)]
    pub fn feed(&self, raw: Box<dyn RawBatch>, fb: usize, frames: usize, stamp: u64, decode: DecodeInto) {
        self.feed_batch(raw, fb, 0..frames, (stamp, None), decode, None);
    }

    /// From the acquisition thread: the first `frames` frames of `fb` bytes in `raw`, recorded under settings stamp
    /// `stamp`, for the workers (and the helper) to claim, decoded with `decode` or, without per-frame measurements,
    /// drawn from their words with `codes` (`draw_frame_codes`). The buffer comes back through `take_buffer` when the
    /// batch is done. Frames nobody claims are counted as dropped. The unit feeds with `feed_with`.
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

    /// `feed_codes` for frames `frames` (those before its start stay out: frame 0 of a batch, and a frame 1 without
    /// its edge, docs/protocols/acquisition-start.md), and `deliver` run by the UI right after the merge that brings the batch's
    /// frames: the newest frame of the batch reaches the screen in the same picture as the others
    /// (docs/fork/stutter.md).
    // Buffer geometry, identity, decoder and one delivery form a single batch submission.
    #[allow(clippy::too_many_arguments)]
    pub fn feed_with(
        &self,
        raw: Box<dyn RawBatch>,
        fb: usize,
        frames: std::ops::Range<usize>,
        identity: oos_core::change::Identity,
        decode: DecodeInto,
        codes: Option<CodesInto>,
        deliver: Deliver,
    ) {
        let b = self.feed_batch(raw, fb, frames, (identity.epoch(), Some(identity)), decode, codes);
        *oos_hal::sync::lock(&b.deliver) = Some(deliver);
    }

    /// Submit immutable preparation in record order; decoding always runs before drawing.
    #[cfg(any(target_os = "linux", test))]
    pub fn feed_prepared(
        &self,
        raw: Box<dyn RawBatch>,
        fb: usize,
        frames: std::ops::Range<usize>,
        identity: oos_core::change::Identity,
        decode: IndexedDecodeInto,
        deliver: Deliver,
    ) {
        let batch = self.feed_batch(
            raw,
            fb,
            frames,
            (identity.epoch(), Some(identity)),
            decoder::Decoder::Prepared(decode),
            None,
        );
        *oos_hal::sync::lock(&batch.deliver) = Some(deliver);
    }

    /// Submit the raw buffer once and share its lease across indexed worker jobs.
    pub(super) fn feed_batch(
        &self,
        raw: Box<dyn RawBatch>,
        fb: usize,
        frames: std::ops::Range<usize>,
        header: (u64, Option<oos_core::change::Identity>),
        decode: impl Into<decoder::Decoder>,
        codes: Option<CodesInto>,
    ) -> Arc<Batch> {
        let (stamp, identity) = header;
        let first = frames.start;
        let frames = frames.end.min(raw.bytes().len().checked_div(fb).unwrap_or(0));
        static IDS: AtomicU64 = AtomicU64::new(0);
        let id = IDS.fetch_add(1, Ordering::Relaxed) + 1;
        let b = Arc::new(Batch {
            lease: Lease { raw: Some(raw), home: self.home.clone() },
            fb,
            frames,
            decode: decode.into(),
            codes,
            header: Header::new(stamp, identity),
            next: AtomicUsize::new(first.min(frames)),
            counters: self.counters,
            live: self.live.clone(),
            deliver: Mutex::new(None),
            ready: self.ready.clone(),
            id,
            fed: std::time::Instant::now(),
        });
        self.live.fetch_add(1, Ordering::Acquire);
        {
            let mut open = oos_hal::sync::lock(&self.open);
            open.retain(|w| w.strong_count() > 0);
            open.push_back(Arc::downgrade(&b));
        }
        for w in &oos_hal::sync::lock(&self.workers).senders {
            // full only if the workers are far behind: the others take its frames
            if let Err(TrySendError::Disconnected(_)) = w.try_send(b.clone()) {
                eprintln!("oos-ui: deposit worker gone");
            }
        }
        b
    }
}

pub(super) fn same_pair(a: Option<oos_core::change::Identity>, b: Option<oos_core::change::Identity>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => (a.gen(), a.epoch()) == (b.gen(), b.epoch()),
        (None, None) => true,
        _ => false,
    }
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
        drop(v);
        pipe.release();
        return;
    };
    let layout = oos_core::change::Layout::new((w, h), (0, 0), (0, 0), None, 0, 0);
    let key = oos_core::change::WorkerViewKey::of(m, layout, &crate::model::config());
    let mapping = v.key.as_ref() != Some(&key);
    let statistics = identity.is_some_and(|id| v.identity.map(|old| old.stats_gen()) != Some(id.stats_gen()));
    if mapping || statistics {
        crate::pacing::VIEWS.fetch_add(1, Ordering::Relaxed);
        v.gen += u64::from(mapping);
        v.view = Some(Arc::new(View { gen: v.gen, m: m.clone(), interp, w, h }));
        v.key = Some(key);
    }
    #[cfg(test)]
    {
        v.stamp = m.hw_stamp;
    }
    if let Some(id) = identity {
        v.identity = Some(id);
    }
}

/// Empty maps for view `v`, reusing `spare` when it has the size.
pub(super) fn fresh_acc(spare: Option<Acc>, v: &View) -> Acc {
    let mut a = match spare {
        Some(mut a) if (a.ph.w, a.ph.h) == (v.w, v.h) => {
            a.reset(v.gen);
            a
        }
        _ => Acc::new(v.gen, v.w, v.h),
    };
    a.used = std::array::from_fn(|c| v.m.ch[c].on);
    for (map, used) in a.ph.map.iter_mut().zip(a.used) {
        if used {
            map.resize(v.w * v.h, 0);
        } else {
            *map = Vec::new();
        }
    }
    a.folds = crate::framestats::folds_for(&v.m);
    a
}
