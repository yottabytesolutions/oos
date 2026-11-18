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
        self.submit(raw, fb, frames, (identity.epoch(), Some(identity)), decode.into(), codes, newest).1
    }

    #[cfg(any(target_os = "linux", test))]
    pub fn feed_prepared(
        &self,
        raw: Box<dyn RawBatch>,
        fb: usize,
        frames: std::ops::Range<usize>,
        identity: oos_core::change::Identity,
        decode: IndexedDecodeInto,
        newest: Option<Arc<Frame>>,
    ) -> Option<Arc<Frame>> {
        self.submit(
            raw,
            fb,
            frames,
            (identity.epoch(), Some(identity)),
            decoder::Decoder::Prepared(decode),
            None,
            newest,
        )
        .1
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
        self.submit(raw, fb, frames, header, decode.into(), codes, None).0
    }

    #[allow(clippy::too_many_arguments)] // All arguments are immutable batch inputs.
    fn submit(
        &self,
        raw: Box<dyn RawBatch>,
        fb: usize,
        frames: std::ops::Range<usize>,
        header: (u64, Option<oos_core::change::Identity>),
        decode: decoder::Decoder,
        codes: Option<CodesInto>,
        newest: Option<Arc<Frame>>,
    ) -> (Arc<Batch>, Option<Arc<Frame>>) {
        let (stamp, identity) = header;
        let first = frames.start;
        let frames = frames.end.min(raw.bytes().len().checked_div(fb).unwrap_or(0));
        let mut b = Arc::new(Batch {
            lease: Lease { raw: Some(raw), home: self.home.clone() },
            fb,
            frames,
            decode,
            codes,
            header: Header::new(stamp, identity),
            view_gen_at_submit: oos_hal::sync::lock(&self.view).gen,
            next: AtomicUsize::new(first.min(frames)),
            counters: self.counters,
            live: self.live.clone(),
            newest,
            fed: std::time::Instant::now(),
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
