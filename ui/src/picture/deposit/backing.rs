// SPDX-License-Identifier: GPL-2.0-or-later
//! Exclusive backing admission. No UI caller waits and no wait creates storage.

use super::*;
use std::sync::{Condvar, Mutex, Weak};

const LIMIT: usize = 5;

pub(super) struct Lease {
    state: Weak<Mutex<Slot>>,
    space: Weak<Condvar>,
}

impl Lease {
    pub(super) fn belongs_to(&self, state: &Arc<Mutex<Slot>>) -> bool {
        self.state.as_ptr() == Arc::as_ptr(state)
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        if let Some(state) = self.state.upgrade() {
            let mut state = oos_hal::sync::lock(&state);
            state.backings -= 1;
            if let Some(space) = self.space.upgrade() {
                space.notify_all();
            }
        }
    }
}

/// Only the producer may call this. Reset/close cancels a waiting old owner.
/// Current production intake remains unchanged until lifecycle qualification.
pub(super) fn acquire(
    slot: &Handoff,
    gen: u64,
    sizes: PerMap<(usize, usize)>,
    identity: Option<Identity>,
    used: [bool; NCH],
) -> Option<Deposit> {
    let mut state = oos_hal::sync::lock(&slot.state);
    while state.spare.iter().all(|d| d.backing.is_none())
        && state.backings >= LIMIT
        && !state.closed
        && gen >= state.reset_before
    {
        #[cfg(test)]
        {
            state.backing_waiting = true;
            slot.space.notify_all();
        }
        state = slot.space.wait(state).unwrap_or_else(std::sync::PoisonError::into_inner);
    }
    #[cfg(test)]
    {
        state.backing_waiting = false;
    }
    if state.closed || gen < state.reset_before {
        return None;
    }
    let spare = if state.backings >= LIMIT {
        state.spare.iter().rposition(|d| d.backing.is_some()).map(|i| state.spare.swap_remove(i))
    } else {
        state.spare.pop()
    };
    let needs_lease = spare.as_ref().is_none_or(|d| d.backing.is_none());
    if needs_lease {
        state.backings += 1;
    }
    drop(state);
    let mut deposit = spare.unwrap_or_else(|| Deposit::new_picture(gen, sizes, identity, used));
    if needs_lease {
        deposit.backing = Some(Lease { state: Arc::downgrade(&slot.state), space: Arc::downgrade(&slot.space) });
    }
    let same_channels = (0..NCH).all(|c| deposit.planes.iter().any(|b| b.channel == c) == used[c]);
    if deposit.sizes() != sizes || !same_channels {
        // Drop the exclusive old payload before allocating its replacement.
        deposit.planes = Vec::new();
        drop(deposit.maps.zoom.take());
        deposit.maps = Maps::new(gen, sizes, used);
        deposit.planes = DiffBand::bands(sizes.main(), used);
    }
    deposit.maps.gen = gen;
    deposit.reset(identity);
    Some(deposit)
}

#[cfg(test)]
pub(super) fn wait_for_backing(slot: &Handoff) {
    let state = oos_hal::sync::lock(&slot.state);
    let (state, _) = slot
        .space
        .wait_timeout_while(state, crate::ui::lifecycle::test_wait::HANG_GUARD, |s| !s.backing_waiting)
        .unwrap();
    assert!(state.backing_waiting);
}

#[cfg(test)]
mod tests;
