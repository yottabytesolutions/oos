// SPDX-License-Identifier: GPL-2.0-or-later

//! Batch dispatch in the acquisition loop. Buffer acceptance precedes matcher learning.

use crate::board::{
    dho::{cores::Cores, matching},
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
    newest: (u64, bool, Option<Frame>),
    identity: Identity,
}

impl<'a> Readout<'a> {
    pub(super) fn new(
        pipe: &'a Pipe,
        settings: (&'a Settings, &'a hz::Params),
        geometry: (usize, usize, bool),
        newest: (u64, bool, Option<Frame>),
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
        // Without a replacement buffer the neighbours cannot draw, so only the newest may teach.
        let first = if next.is_some() { 0 } else { avail - 1 };
        let probe = self.newest.1;
        let (matched, newest) = matching::prepare(
            &dma.bytes()[first * fb..avail * fb],
            (fb, avail - first, frame0_drop && first == 0),
            self.settings,
            self.params,
            self.newest,
            cores,
        );
        let newest = tag(newest);
        match next {
            Some(next) => {
                let full = std::mem::replace(dma, next);
                // Publish the newest after its neighbours, so the screen receives one complete batch picture.
                matching::feed(
                    self.pipe,
                    (Box::new(full), fb, avail - 1),
                    self.identity,
                    (self.settings.clone(), *self.params, probe),
                    matched,
                    deliver(newest),
                );
                Vec::new()
            }
            None => {
                crate::drawpipe::Counters::add(&crate::drawpipe::counters().dropped, avail as u64 - 1);
                vec![newest]
            }
        }
    }
}
