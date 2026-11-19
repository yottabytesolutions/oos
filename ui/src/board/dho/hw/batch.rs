// SPDX-License-Identifier: GPL-2.0-or-later

//! Batch dispatch in the acquisition loop. Buffer acceptance precedes matcher learning.

use crate::board::{
    dho::{cores::Cores, frame1, matching, switches::HeaderProbe},
    Frame,
};
use crate::drawpipe::{Pipe, RawBatch, TapReservation};
use oos_core::change::Identity;
use oos_hal::dho::{hz, seq::Settings};
use std::sync::Arc;
use std::time::Instant;

mod tap;
#[cfg(target_os = "linux")]
pub(super) use tap::{source_reservation, storage};
#[cfg(test)]
mod tests;

pub(super) fn deposit_plan(s: &Settings, p: &hz::Params, id: Identity, route: bool) -> (Option<&'static Pipe>, u32) {
    let Some(pipe) = crate::drawpipe::pipe(crate::drawpipe::Access::Existing) else { return (None, s.frames) };
    let live = (p.memsel <= crate::compress::MAX_DEPTH).then(|| (p.rate, oos_hal::dho::seq::compress_k(s, p)));
    if let Some(limit) = (route && pipe.accepting()).then(|| pipe.read_limit(id, live)).flatten() {
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
    average: bool,
    funded_single_record: bool,
) -> Option<&Pipe> {
    pipe.filter(|pipe| !average && (count > 1 || (count == 1 && funded_single_record)) && pipe.accepting())
}

pub(super) struct Readout<'a> {
    pipe: &'a Pipe,
    settings: &'a Settings,
    params: &'a hz::Params,
    geometry: (usize, usize, bool),
    newest: (u64, HeaderProbe, Option<Frame>),
    batch: (Identity, Instant),
    reserved: Option<Arc<TapReservation>>,
}

impl<'a> Readout<'a> {
    pub(super) fn new(
        pipe: &'a Pipe,
        settings: (&'a Settings, &'a hz::Params),
        geometry: (usize, usize, bool),
        newest: (u64, HeaderProbe, Option<Frame>),
        batch: (Identity, Instant),
    ) -> Self {
        Self { pipe, settings: settings.0, params: settings.1, geometry, newest, batch, reserved: None }
    }

    pub(super) fn reserved(mut self, reservation: Option<Arc<TapReservation>>) -> Self {
        self.reserved = reservation;
        self
    }

    pub(super) fn dispatch<T: RawBatch + 'static>(
        self,
        dma: &mut T,
        next: Option<T>,
        cores: Option<&mut Cores>,
        tag: impl FnOnce(Frame) -> Frame,
    ) -> Vec<Frame> {
        let (fb, avail, frame0_drop) = self.geometry;
        let counters = crate::drawpipe::counters();
        if avail == 0 || fb == 0 || avail > dma.bytes().len() / fb {
            counters.discard(crate::drawpipe::Lost::Error, avail as u64);
            return Vec::new();
        }
        // frame 0, and a frame 1 without its edge at the trigger point, reach neither the workers nor the matcher
        let start = frame1::first_kept(dma.bytes(), fb, avail, frame0_drop, (self.settings, self.params));
        counters.lose(crate::drawpipe::Lost::Frame0, u64::from(frame0_drop && avail > 1));
        // Without a replacement buffer the neighbours cannot draw, so only the newest may teach.
        let first = if next.is_some() { 0 } else { avail - 1 };
        let (probe, deposit) = (self.newest.1, next.is_some().then_some(self.batch.0));
        let Ok((storage, reservation)) =
            tap::reserve(self.pipe, self.settings, fb, self.newest.2.as_ref(), deposit, self.reserved)
                .inspect_err(|&lost| counters.discard(lost, avail.saturating_sub(start) as u64))
        else {
            return Vec::new();
        };
        let (matched, newest) = matching::prepare_into(
            &dma.bytes()[first * fb..avail * fb],
            (fb, avail - first, start.saturating_sub(first)),
            self.settings,
            self.params,
            self.newest,
            cores,
            storage,
        );
        // with two frames the newest is frame 1
        let keep = frame1::newest_kept(&newest, avail, frame0_drop, self.settings);
        let newest = tap::retain(tag(newest), reservation.as_ref());
        match next {
            Some(next) => {
                let full = std::mem::replace(dma, next);
                // Publish the newest after its neighbours, so the screen receives one complete batch picture.
                matching::feed(
                    self.pipe,
                    (Box::new(full), fb, start..if keep { avail } else { avail - 1 }),
                    self.batch,
                    (self.settings.clone(), *self.params, probe),
                    matched,
                    (keep.then(|| Arc::new(newest)), reservation),
                )
                .into_iter()
                .map(Arc::unwrap_or_clone)
                .collect()
            }
            None => {
                drop(reservation.inspect(|charge| charge.fallback()));
                fallback(counters, avail, start, keep.then_some(newest))
            }
        }
    }
}

fn fallback(counters: &crate::drawpipe::Counters, avail: usize, start: usize, newest: Option<Frame>) -> Vec<Frame> {
    crate::drawpipe::Counters::add(&counters.dropped, avail as u64 - 1);
    counters.lose(crate::drawpipe::Lost::Buffer, (avail - 1).saturating_sub(start) as u64);
    newest.into_iter().collect()
}

#[cfg(target_os = "linux")]
pub(super) fn summarize(frames: &mut [Frame]) {
    if let Some(frame) = frames.last_mut() {
        *frame.basic = std::array::from_fn(|c| crate::board::BasicStats::of(&frame.ch[c]));
    }
}
