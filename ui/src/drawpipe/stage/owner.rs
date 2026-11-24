// SPDX-License-Identifier: GPL-2.0-or-later
//! Immutable pre-first-use intake. Five counted backing leases remain authoritative.
use super::super::*;
use crate::picture::deposit::{Deposit, Handoff};
use crate::picture::{DepositWindow, FirstDraw, PerMap};
use crate::wave::Phosphor;
use oos_core::change::Identity;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
mod compose;

const PICTURE_INPUTS: usize = 5;

#[derive(Clone)]
struct OwnerContext {
    slot: Arc<Handoff>,
    view: Arc<Mutex<ViewState>>,
    counters: &'static Counters,
}
#[derive(Default)]
pub(crate) struct Owner {
    context: Option<OwnerContext>,
    queue: VecDeque<Deposit>,
    active: bool,
    wake: Option<Arc<crate::system::intake::Wake>>,
}
impl Owner {
    pub(crate) fn take_wake(&mut self) -> Option<Arc<crate::system::intake::Wake>> {
        self.wake.take()
    }
    pub(crate) fn set_wake(&mut self, wake: Arc<crate::system::intake::Wake>) {
        self.wake = Some(wake);
    }
    pub(crate) fn clear(&mut self) {
        if let Some(ctx) = &self.context {
            ctx.slot.owner_enable(false, None);
        }
        self.discard_queued(false);
        self.context = None;
        self.active = false;
    }

    pub(crate) fn retire_inputs(&mut self, pipe: &Pipe, keep_current: bool) {
        self.discard_queued(keep_current);
        let identity = oos_hal::sync::lock(&pipe.view).identity;
        if let Some(deposit) = pipe.slot.take_unusable(identity, keep_current) {
            reject_deposit(&pipe.slot, pipe.counters, identity, deposit);
            pipe.wake();
        }
    }

    fn discard_queued(&mut self, keep_current: bool) {
        if self.queue.is_empty() {
            return;
        }
        let ctx = self.context.as_ref().expect("queued deposits belong to a bound owner");
        let id = oos_hal::sync::lock(&ctx.view).identity;
        for _ in 0..self.queue.len() {
            let d = self.queue.pop_front().expect("bounded existing input");
            if keep_current && same_pair(d.identity, id) {
                self.queue.push_back(d);
                continue;
            }
            reject_deposit(&ctx.slot, ctx.counters, id, d);
        }
    }

    fn stage_pending(&mut self, pipe: &Pipe) {
        pipe.slot.owner_rearm(); // Remote drains shared Wake before this check, never after it.
        for _ in self.queue.len()..PICTURE_INPUTS {
            let Some(mut slot) = pipe.slot.stage() else { break };
            self.stage_deposit(&mut slot);
            drop(slot);
            pipe.wake();
        }
    }

    fn stage_deposit(&mut self, slot: &mut crate::picture::deposit::Claim<'_>) {
        if let Some(mut deposit) = slot.deposit.take() {
            // Order bands as the input arrives. Identity is checked at the picture step.
            deposit.sort_bands();
            self.queue.push_back(deposit);
        }
    }
}
fn reject_deposit(slot: &Handoff, counters: &Counters, identity: Option<Identity>, deposit: Deposit) {
    slot.settle_taps(&deposit.taps, None);
    deposit.reject(identity, counters);
    slot.recycle(deposit);
}

impl Drop for Owner {
    fn drop(&mut self) {
        self.clear();
    }
}
impl Owner {
    pub(crate) fn intake_owned(&mut self, pipe: &Pipe, active: bool) {
        if !active || self.wake.is_none() {
            self.active = false;
            pipe.slot.owner_enable(false, None);
            return;
        }
        self.bind(pipe);
        self.active = true;
        pipe.slot.owner_enable(true, self.wake.clone());
        self.stage_pending(pipe);
    }
    fn bind(&mut self, pipe: &Pipe) {
        if self.context.as_ref().is_some_and(|c| !Arc::ptr_eq(&c.slot, &pipe.slot)) {
            self.clear();
        }
        if self.context.is_none() {
            self.context =
                Some(OwnerContext { slot: pipe.slot.clone(), view: pipe.view.clone(), counters: pipe.counters });
        }
    }

    fn seal(&mut self, pipe: &Pipe) {
        if self.queue.len() >= PICTURE_INPUTS {
            return;
        }
        pipe.slot.owner_rearm();
        if let Some(mut slot) = pipe.slot.stage() {
            self.stage_deposit(&mut slot);
            drop(slot);
            pipe.wake();
        }
    }

    #[cfg(test)]
    pub(crate) fn compose_owned(
        &mut self,
        pipe: &Pipe,
        ph: &mut Phosphor,
        clear: &mut bool,
        stats: Stats<'_>,
        window: Option<&mut DepositWindow>,
    ) -> Accepted {
        let mut first = if *clear { FirstDraw::Minimum } else { FirstDraw::Keep };
        let accepted = self.compose_picture(pipe, PerMap::new(ph, None), &mut first, stats, window);
        *clear = first == FirstDraw::Minimum;
        accepted
    }

    pub(crate) fn compose_picture(
        &mut self,
        pipe: &Pipe,
        maps: PerMap<&mut Phosphor>,
        first: &mut FirstDraw,
        stats: Stats<'_>,
        window: Option<&mut DepositWindow>,
    ) -> Accepted {
        self.bind(pipe);
        if self.active {
            // The pending slot is observed once. Publications after this seal belong to the next picture.
            self.seal(pipe);
            return self.compose(maps, first, stats, window);
        }
        // Synchronous intake keeps the slot occupied until compose can recycle its backing.
        let mut claim = pipe.slot.claim();
        if let Some(slot) = &mut claim {
            self.stage_deposit(slot);
        }
        let accepted = self.compose(maps, first, stats, window);
        if claim.is_some() {
            drop(claim);
            pipe.wake();
        }
        accepted
    }
}

pub(super) fn publish_completed(
    slot: &Handoff,
    b: Arc<Batch>,
    d: Deposit,
    view: &Mutex<ViewState>,
    counters: &Counters,
) -> Option<Deposit> {
    let flight = oos_hal::sync::lock(&b.header.flight).take();
    if flight.is_none() && !slot.owner_enabled() {
        let current = publish_pending(slot, d, view, counters);
        drop(b);
        return current;
    }
    // Jobs are joined by Painter::batch. Drop the completed raw/decoder owner before output capacity wait.
    #[cfg(test)]
    let injected = b.clock.pool_override().is_some();
    drop(b);
    let result = slot.send_timed(d);
    match result {
        Ok(publication) => {
            if let Some(flight) = flight {
                #[cfg(test)]
                let publication = if injected { std::time::Duration::ZERO } else { publication };
                flight.published(publication);
            }
        }
        Err(d) => {
            d.reject(oos_hal::sync::lock(view).identity, counters);
            slot.recycle(d);
        }
    }
    None
}

pub(super) fn publish_pending(
    slot: &Handoff,
    d: Deposit,
    view: &Mutex<ViewState>,
    counters: &Counters,
) -> Option<Deposit> {
    let d = slot.put(d).err()?;
    let identity = oos_hal::sync::lock(view).identity;
    if same_pair(d.identity, identity) {
        Some(d)
    } else {
        d.reject(identity, counters);
        slot.recycle(d);
        None
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
impl Pipe {
    pub(crate) fn wait_owner_output(&self) {
        self.slot.wait_for_sender();
    }
}

#[cfg(test)]
impl Owner {
    pub(crate) fn debug_state(&self) -> String {
        let context = self.context.as_ref().map(|c| {
            let v = oos_hal::sync::lock(&c.view);
            (v.gen, v.mapping_gen, v.identity, c.slot.owner_enabled())
        });
        let inputs = self.queue.iter().map(|d| (d.records, d.identity)).collect::<Vec<_>>();
        format!("enabled={} queue_len={} context={context:?} inputs={inputs:?}", self.active, self.queue.len())
    }
}

#[cfg(test)]
impl Owner {
    pub(crate) fn test_wake(&self) -> Option<Arc<crate::system::intake::Wake>> {
        self.wake.clone()
    }
}
