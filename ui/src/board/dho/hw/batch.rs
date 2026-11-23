// SPDX-License-Identifier: GPL-2.0-or-later

//! Batch dispatch in the acquisition loop. Buffer acceptance precedes matcher learning.

use crate::board::{
    dho::{
        cores::Cores,
        frame1, matching,
        switches::{FrameZero, HeaderProbe},
    },
    Frame,
};
use crate::drawpipe::{clock::StageClock, Pipe, RawBatch, TapReservation};
use oos_core::change::Identity;
use oos_hal::dho::{hz, seq::Settings};
use std::sync::Arc;
#[cfg(test)]
use std::time::Instant;

#[cfg(any(target_os = "linux", test))]
mod admission;
mod tap;
#[cfg(any(target_os = "linux", test))]
pub(super) use admission::grant;
#[cfg(test)]
pub(super) use admission::read_grant;
#[cfg(any(target_os = "linux", test))]
pub(super) use tap::source_reservation;
#[cfg(any(target_os = "linux", test))]
pub(super) use tap::storage;
#[cfg(test)]
mod tests;

pub(super) fn deposit_plan(s: &Settings, p: &hz::Params, id: Identity, route: bool) -> (Option<&'static Pipe>, u32) {
    let Some(pipe) = crate::drawpipe::pipe() else { return (None, s.frames) };
    let live = (p.memsel <= crate::compress::MAX_DEPTH).then(|| (p.rate, oos_hal::dho::seq::compress_k(s, p)));
    let pairs = super::decode::pair_step(s, p) > 1;
    if let Some(limit) = (route && pipe.accepting() && crate::drawpipe::average::input_ok(pipe, id, pairs))
        .then(|| pipe.read_limit(id, live))
        .flatten()
    {
        if !pipe.staged_taps() || tap::admit(pipe, s, p, id) {
            return (Some(pipe), s.frames.min(limit));
        }
    }
    pipe.retire_tap_plan();
    (None, s.frames)
}

#[cfg(any(target_os = "linux", test))]
pub(super) fn readout_pipe(
    pipe: Option<&Pipe>,
    count: usize,
    hardware_average: bool,
    funded_single_record: bool,
) -> Option<&Pipe> {
    pipe.filter(|pipe| !hardware_average && (count > 1 || (count == 1 && funded_single_record)) && pipe.accepting())
}

pub(super) struct Readout<'a> {
    pipe: &'a Pipe,
    settings: &'a Settings,
    params: &'a hz::Params,
    geometry: (usize, usize, FrameZero),
    newest: (u64, HeaderProbe, Option<Frame>),
    batch: (Identity, StageClock),
    reserved: Option<Arc<TapReservation>>,
    trace: Option<Box<crate::drawpipe::route::RouteTrace>>,
}

impl<'a> Readout<'a> {
    pub(super) fn new(
        pipe: &'a Pipe,
        settings: (&'a Settings, &'a hz::Params),
        (fb, avail, frame0): (usize, usize, impl Into<FrameZero>),
        newest: (u64, HeaderProbe, Option<Frame>),
        (identity, clock): (Identity, impl Into<StageClock>),
    ) -> Self {
        Self {
            pipe,
            settings: settings.0,
            params: settings.1,
            geometry: (fb, avail, frame0.into()),
            newest,
            batch: (identity, clock.into()),
            reserved: None,
            trace: None,
        }
    }

    pub(super) fn reserved(mut self, reservation: Option<Arc<TapReservation>>) -> Self {
        self.reserved = reservation;
        self
    }

    pub(super) fn trace(mut self, trace: Option<Box<crate::drawpipe::route::RouteTrace>>) -> Self {
        self.trace = trace;
        self
    }

    pub(super) fn dispatch<T: RawBatch + 'static>(
        self,
        dma: &mut Option<T>,
        cores: Option<&mut Cores>,
        tag: impl FnOnce(Frame) -> Frame,
        canceled: &dyn Fn() -> bool,
    ) -> Vec<Frame> {
        let (fb, avail, policy) = self.geometry;
        let counters = self.pipe.batch_counters();
        let bytes = dma.as_ref().expect("a completed read still owns its DMA destination").bytes();
        if avail == 0 || fb == 0 || avail > bytes.len() / fb {
            counters.discard(crate::drawpipe::Lost::Error, avail as u64);
            return Vec::new();
        }
        let selection =
            frame1::dma_selection((bytes, fb, avail), self.newest.2.as_ref(), policy, (self.settings, self.params));
        counters.lose(crate::drawpipe::Lost::Frame0, u64::from(selection.drop_first));
        if selection.range.is_empty() {
            // Releasing the output slot wakes the coordinator to retry any retained deposit.
            return Vec::new();
        }

        let (seq, probe, raw) = self.newest;
        let from_dma = raw.is_none();
        let Ok((storage, reservation)) =
            tap::reserve(self.pipe, self.settings, fb, raw.as_ref(), Some(self.batch.0), self.reserved, canceled)
                .inspect_err(|&lost| counters.discard(lost, selection.range.len() as u64))
        else {
            return Vec::new();
        };
        let mut newest = raw.unwrap_or(storage);
        newest.seq = seq;
        let full = dma.take().expect("the validated read still owns its DMA destination");
        matching::submit(
            self.pipe,
            (super::header::dump::Captured::wrap(full, self.trace), fb, selection.range, selection.skip_second),
            self.batch,
            (self.settings.clone(), *self.params, probe),
            cores,
            (Some(newest), reservation.clone(), from_dma, |frame| tap::retain(tag(frame), reservation.as_ref())),
        )
        .into_iter()
        .map(Arc::unwrap_or_clone)
        .collect()
    }
}

/// A decoded board record and whether any active channel bypassed core correction.
pub(super) type DecodedRecord = (Frame, bool);

#[cfg(any(target_os = "linux", test))]
pub(super) fn decode_records(
    (match_cores, identity): (bool, Identity),
    (settings, params, probe): (&Settings, &hz::Params, HeaderProbe),
    (bytes, fb, count): (&[u8], usize, usize),
    (frame0, average): (impl Into<FrameZero>, bool),
    raw: Option<DecodedRecord>,
    (seq, cores): (&mut u64, &mut Cores),
) -> (Vec<Frame>, usize) {
    use crate::board::dho::publication;
    let policy = frame0.into();
    let skipped = publication::record_decode(count, average, policy == FrameZero::Drop);
    let mut skip = publication::account_skipped(skipped, average);
    *seq += skip as u64;
    let mut uncorrected = 0;
    let mut last_uncorrected = false;
    let mut frames: Vec<_> = bytes
        .chunks_exact(fb.max(1))
        .take(count)
        .enumerate()
        .skip(skip)
        .filter_map(|(slot, bytes)| {
            *seq += 1;
            let (frame, missed) =
                decode_record(bytes, (settings, params, probe), match_cores.then_some(&mut *cores), *seq);
            if !average
                && count > 1
                && slot < 2
                && frame1::off_trigger_if_checked(
                    (slot, count),
                    policy,
                    (&frame, raw.as_ref().map(|(f, _)| f)),
                    settings,
                )
            {
                skip += 1;
                return None;
            }
            uncorrected += usize::from(missed);
            last_uncorrected = missed;
            Some(publication::tag(Frame { hw_avg: average, ..frame }, identity))
        })
        .collect();
    // A two-slot batch whose newest record failed the edge check cannot publish an older picture.
    if count == 2 && frames.last().is_some_and(|f| f.seq != *seq) {
        crate::drawpipe::counters().lose(crate::drawpipe::Lost::Frame0, 1);
        // The uncorrected metric counts records selected for publication, so this suppressed slot zero is excluded.
        return (Vec::new(), skip + 1);
    }
    let raw = raw.filter(|_| frames.last().is_some_and(|f| f.seq == *seq)).map(|(frame, missed)| {
        uncorrected = uncorrected - usize::from(last_uncorrected) + usize::from(missed);
        frame
    });
    crate::drawpipe::counters().metrics.source_uncorrected(identity, uncorrected);
    frame1::raw_newest(&mut frames, raw, (identity.epoch(), identity.gen()));
    (frames, skip)
}

#[cfg(any(target_os = "linux", test))]
pub(super) fn decode_record(
    bytes: &[u8],
    (s, p, probe): (&Settings, &hz::Params, HeaderProbe),
    cores: Option<&mut Cores>,
    seq: u64,
) -> DecodedRecord {
    let mut frame = Frame::default();
    let uncorrected = super::decode::decode_into_with(bytes, s, p, seq, &mut frame, probe, cores);
    (frame, uncorrected)
}

#[cfg(any(target_os = "linux", test))]
pub(super) fn summarize(frames: &mut [Frame]) {
    if let Some(frame) = frames.last_mut() {
        *frame.basic = std::array::from_fn(|c| crate::board::BasicStats::of(&frame.ch[c]));
    }
}

/// Read the sticky trigger status and clear it before arming a successor.
#[cfg(any(target_os = "linux", test))]
pub(super) fn read_trigger(bar: &mut impl oos_hal::dho::bus::Bus) -> Option<bool> {
    use oos_hal::dho::scu;
    let value = bar.rd(0x4034);
    bar.apply(&scu::clear_status(value));
    match scu::fired(value) {
        scu::Fired::Trigger => Some(true),
        scu::Fired::Forced => Some(false),
        scu::Fired::None => None,
    }
}

/// Read the logic analyser data before the next register operation can replace this record.
#[cfg(any(target_os = "linux", test))]
pub(super) fn read_la(
    state: &mut oos_hal::dho::seq::State,
    (settings, params): (&Settings, &hz::Params),
    (bar, c2h, clock): (
        &mut impl oos_hal::dho::bus::Bus,
        &mut impl std::io::Read,
        &impl oos_hal::dho::seq::time::Clock,
    ),
    fails: &mut u32,
    status: impl FnOnce(String),
) -> Option<Vec<u16>> {
    if settings.la.is_none() || *fails >= 3 {
        return None;
    }
    match oos_hal::dho::seq::time::acquire_la_with_clock(state, settings, params, bar, c2h, |_| {}, clock) {
        Ok(words) => {
            *fails = 0;
            Some(words)
        }
        Err(error) => {
            *fails += 1;
            status(format!("LA read ({} of 3): {}", *fails, error));
            None
        }
    }
}
