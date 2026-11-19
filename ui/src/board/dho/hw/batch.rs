// SPDX-License-Identifier: GPL-2.0-or-later

//! Batch dispatch in the acquisition loop. Buffer acceptance precedes matcher learning.

use crate::board::{
    dho::{cores::Cores, frame1, matching, switches::HeaderProbe},
    Frame,
};
use crate::drawpipe::{Deliver, Pipe, RawBatch};
use oos_core::change::Identity;
use oos_hal::dho::{hz, seq::Settings};

#[cfg(test)]
mod tests;

pub(super) struct Readout<'a> {
    pipe: &'a Pipe,
    settings: &'a Settings,
    params: &'a hz::Params,
    geometry: (usize, usize, bool),
    newest: (u64, HeaderProbe, Option<Frame>),
    identity: Identity,
}

impl<'a> Readout<'a> {
    pub(super) fn new(
        pipe: &'a Pipe,
        settings: (&'a Settings, &'a hz::Params),
        geometry: (usize, usize, bool),
        newest: (u64, HeaderProbe, Option<Frame>),
        identity: Identity,
    ) -> Self {
        Self { pipe, settings: settings.0, params: settings.1, geometry, newest, identity }
    }

    pub(super) fn dispatch<T: RawBatch + 'static>(
        self,
        dma: &mut T,
        next: Option<T>,
        cores: Option<&mut Cores>,
        tag: impl FnOnce(Frame) -> Frame,
        deliver: impl FnOnce(Frame) -> Deliver,
    ) -> Vec<Frame> {
        let (fb, avail, frame0_drop) = self.geometry;
        // frame 0, and a frame 1 without its edge at the trigger point, reach neither the workers nor the matcher
        let start = frame1::first_kept(dma.bytes(), fb, avail, frame0_drop, (self.settings, self.params));
        let counters = crate::drawpipe::counters();
        counters.lose(crate::drawpipe::Lost::Frame0, u64::from(frame0_drop && avail > 1));
        // Without a replacement buffer the neighbours cannot draw, so only the newest may teach.
        let first = if next.is_some() { 0 } else { avail - 1 };
        let probe = self.newest.1;
        let (matched, newest) = matching::prepare(
            &dma.bytes()[first * fb..avail * fb],
            (fb, avail - first, start.saturating_sub(first)),
            self.settings,
            self.params,
            self.newest,
            cores,
        );
        // with two frames the newest is frame 1
        let keep = frame1::newest_kept(&newest, avail, frame0_drop, self.settings);
        let newest = tag(newest);
        match next {
            Some(next) => {
                let full = std::mem::replace(dma, next);
                // Publish the newest after its neighbours, so the screen receives one complete batch picture.
                matching::feed(
                    self.pipe,
                    (Box::new(full), fb, start..avail - 1),
                    self.identity,
                    (self.settings.clone(), *self.params, probe),
                    matched,
                    if keep { deliver(newest) } else { Box::new(|| {}) },
                );
                Vec::new()
            }
            None => {
                crate::drawpipe::Counters::add(&counters.dropped, avail as u64 - 1);
                counters.lose(crate::drawpipe::Lost::Buffer, (avail - 1).saturating_sub(start) as u64);
                if keep {
                    vec![newest]
                } else {
                    Vec::new()
                }
            }
        }
    }
}
