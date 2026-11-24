// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;
use crate::ui::lifecycle::test_wait::HANG_GUARD;
use std::sync::mpsc::channel;

fn acquire(slot: &Handoff, gen: u64) -> Deposit {
    slot.acquire(gen, (8, 8), None, [true; NCH]).unwrap()
}

#[test]
fn five_backings_block_then_recycle_reset_close_or_last_drop_release_waiter() {
    for action in 0..4 {
        let slot = Handoff::default();
        let mut held: Vec<_> = (0..5).map(|_| acquire(&slot, 0)).collect();
        std::thread::scope(|threads| {
            let (done, result) = channel();
            let slot = &slot;
            let worker = threads.spawn(move || {
                done.send(slot.acquire(0, (8, 8), None, [true; NCH])).unwrap();
            });
            wait_for_backing(slot);
            assert!(result.try_recv().is_err());
            assert_eq!(oos_hal::sync::lock(&slot.state).backings, 5);
            match action {
                0 => slot.recycle(held.pop().unwrap()),
                1 => drop(slot.reset(1)),
                2 => slot.close(),
                _ => drop(held.pop()),
            }
            let next = result.recv_timeout(HANG_GUARD).unwrap();
            assert_eq!(next.is_some(), action == 0 || action == 3);
            drop(next);
            worker.join().unwrap();
        });
        drop(held);
        assert_eq!(oos_hal::sync::lock(&slot.state).backings, 0);
    }
}

#[test]
fn reset_drops_cached_outside_lock_and_pending_claimed_late_returns_keep_exact_ownership() {
    let slot = Handoff::default();
    let cached = acquire(&slot, 0);
    slot.recycle(cached);
    assert!(slot.put(acquire(&slot, 0)).is_ok());
    let mut claim = slot.claim().unwrap();
    let open = acquire(&slot, 0);
    slot.recycle(acquire(&slot, 0));
    assert_eq!(oos_hal::sync::lock(&slot.state).backings, 3);
    assert!(slot.reset(1).is_none());
    assert_eq!(oos_hal::sync::lock(&slot.state).backings, 2);
    slot.recycle(claim.deposit.take().unwrap());
    drop(claim);
    let reused = acquire(&slot, 1);
    assert!(reused.matches(1, 8, 8));
    assert!(slot.put(open).is_err(), "old owner cannot publish across reset");
    drop(reused);
    assert_eq!(oos_hal::sync::lock(&slot.state).backings, 0);
}

#[test]
fn pending_reset_and_rejected_publication_are_dropped_by_caller_after_unlock() {
    let slot = Handoff::default();
    assert!(slot.put(acquire(&slot, 0)).is_ok());
    let rejected = slot.put(acquire(&slot, 0)).err().unwrap();
    assert_eq!(oos_hal::sync::lock(&slot.state).backings, 2);
    drop(rejected);
    let pending = slot.reset(1).unwrap();
    assert_eq!(oos_hal::sync::lock(&slot.state).backings, 1);
    drop(pending);
    assert_eq!(oos_hal::sync::lock(&slot.state).backings, 0);
}

#[test]
fn legacy_unleased_spare_cannot_hide_reusable_counted_backing_and_closed_put_refuses() {
    let slot = Handoff::default();
    let mut held: Vec<_> = (0..5).map(|_| acquire(&slot, 0)).collect();
    let reusable = held.pop().unwrap();
    let pointer = reusable.planes[0].test_backing();
    slot.recycle(reusable);
    // Before route cutover, legacy parent callers can still return an unleased
    // set. Its position must not hide an already admitted reusable set.
    slot.recycle(Deposit::new(0, (8, 8), None, [true; NCH]));
    let reused = acquire(&slot, 0);
    assert_eq!(reused.planes[0].test_backing(), pointer);
    assert_eq!(oos_hal::sync::lock(&slot.state).backings, 5);
    slot.close();
    let rejected = slot.put(reused).err().unwrap();
    assert_eq!(oos_hal::sync::lock(&slot.state).backings, 5);
    drop(rejected);
    drop(held);
    assert_eq!(oos_hal::sync::lock(&slot.state).backings, 0);
}

#[test]
fn foreign_return_retires_original_permit_without_adopting_it_into_new_handoff() {
    let original = Handoff::default();
    let next = Handoff::default();
    let returned = acquire(&original, 0);
    assert_eq!(oos_hal::sync::lock(&original.state).backings, 1);
    next.recycle(returned);
    assert_eq!(oos_hal::sync::lock(&original.state).backings, 0);
    assert_eq!(oos_hal::sync::lock(&next.state).backings, 0);
    assert!(next.spare().is_none());
    let own = acquire(&next, 0);
    assert_eq!(oos_hal::sync::lock(&next.state).backings, 1);
    drop(own);
    assert_eq!(oos_hal::sync::lock(&next.state).backings, 0);
}
