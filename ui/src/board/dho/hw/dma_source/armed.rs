// SPDX-License-Identifier: GPL-2.0-or-later

use super::{apply_count, ArmPlan, ArmedBatch, DmaSource, Grant, LoopClock, Pipe, TapReservation};
use crate::board::{dho::hw::acq::Replay, HwShared};
use oos_hal::dho::{bus::DmaBuf, seq::Settings};
use std::sync::Arc;

#[derive(Default)]
pub(in crate::board::dho::hw) struct Successor(Option<ArmedBatch>);

impl Successor {
    pub(in crate::board::dho::hw) fn as_ref(&self) -> Option<&ArmedBatch> {
        self.0.as_ref()
    }

    pub(in crate::board::dho::hw) fn is_some(&self) -> bool {
        self.0.is_some()
    }

    fn arm(&mut self, batch: ArmedBatch) {
        debug_assert!(self.0.is_none());
        self.0 = Some(batch);
    }

    fn consume(&mut self) -> Option<Arc<TapReservation>> {
        self.0.take().and_then(|batch| batch.tap)
    }

    pub(in crate::board::dho::hw) fn disarm(&mut self, replay: &mut Replay, out: &HwShared, stop: impl FnOnce()) {
        if let Some(count) = self.0.take().map(|batch| replay.successor_slots(&batch.settings)) {
            stop();
            replay.invalidate(out, count);
        }
    }
}

impl ArmPlan<'_> {
    pub(in crate::board::dho::hw) fn prepare(
        &self,
        dma: &mut DmaSource<DmaBuf>,
        armed: &mut Successor,
        clock: &impl LoopClock,
        obsolete: impl Fn() -> bool,
        disarm: impl FnOnce(&mut Successor),
    ) -> Option<(bool, Option<Arc<TapReservation>>)> {
        if !dma.ready(self.identity, clock, &obsolete) {
            return None;
        }
        if armed.as_ref().is_some_and(|a| a.matches(self.settings, self.pair(), self.route, self.deposit)) {
            return (!obsolete()).then(|| (true, armed.consume()));
        }
        disarm(armed);
        let tap = super::super::batch::source_reservation(self.deposit, self.identity, &obsolete).ok()?;
        (!obsolete()).then_some((false, tap))
    }

    /// Read LA and raw replay before calling this. An owned destination and any funded tap are required before arming.
    /// When the current buffer will be loaned, keep a reserved spare available even if ahead arm is deferred.
    pub(in crate::board::dho::hw) fn prearm(
        &self,
        dma: &mut DmaSource<DmaBuf>,
        armed: &mut Successor,
        loan: Option<&Pipe>,
        successor: Option<Grant>,
        obsolete: impl Fn() -> bool,
        arm: impl FnOnce(&Settings, Option<&TapReservation>) -> bool,
    ) -> Option<DmaBuf> {
        let Grant::Overlapping(frames) = successor? else { return None };
        if obsolete() {
            return None;
        }
        let next = loan.and_then(|pipe| dma.spare(pipe));
        if loan.is_some() && next.is_none() {
            // No free destination: defer prearm until a returned buffer is owned.
            return None;
        }
        let funded = self.deposit.filter(|pipe| pipe.staged_taps());
        let tap = funded.and_then(|pipe| pipe.try_prearm_tap(self.identity));
        if funded.is_none() || tap.is_some() {
            let mut successor = self.settings.clone();
            apply_count(&mut successor, frames);

            if !obsolete() && arm(&successor, tap.as_deref()) {
                armed.arm(ArmedBatch {
                    settings: successor,
                    pair: self.pair(),
                    route: self.route,
                    deposit: self.deposit.is_some(),
                    tap,
                });
            }
        }
        next
    }
}
