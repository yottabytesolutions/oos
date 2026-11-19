// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;
use crate::picture::deposit::{Handoff, Tap, TapProjection, TapRead};

#[test]
fn fifo_and_final_reader_slots_are_independently_bounded() {
    let handoff = Arc::new(Handoff::default());
    let id = Identity::default();
    handoff.set_tap_identity(id);
    handoff.fund_taps(Share::checked(1 << 20, 0, 1 << 20, 16, Arc::new(())).unwrap(), Duration::from_secs(1)).unwrap();
    assert!(handoff.admit_taps(id, [0; 6], 1, Duration::from_secs(1)));
    // ceil(T/1s)+3 = 4: eight FIFO entries, ten total frame slots.
    assert_eq!(handoff.tap_drain(), 4);
    let mut held = Vec::new();
    for _ in 0..8 {
        let charge = handoff.reserve_plan(0, id).unwrap();
        let tap = Tap {
            frame: Arc::new(crate::board::Frame::default()),
            identity: Some(id),
            projection: TapProjection::Unpainted,
            folds: None,
        };
        handoff.send_tap(tap, 0, Some(&charge)).unwrap();
        held.push(charge);
    }
    assert_eq!(handoff.tap_count(), 8);
    for _ in 0..2 {
        assert!(matches!(handoff.take_tap(None, Some(id), None, true), TapRead::Ready(_)));
        held.push(handoff.reserve_plan(0, id).unwrap());
    }
    // Two readers plus eight pending/queued grants exhaust all ten live slots.
    std::thread::scope(|threads| {
        let waiting = threads.spawn(|| handoff.reserve_plan(0, id));
        handoff.wait_for_sender();
        drop(held.remove(0));
        // There is still no FIFO slot: release another consumed reader and dequeue once.
        assert!(matches!(handoff.take_tap(None, Some(id), None, true), TapRead::Ready(_)));
        assert!(waiting.join().unwrap().is_some());
    });
    assert!(handoff.reserve_tap(1, 0, Some(id)).is_none());
    handoff.close();
    drop(held);
}

#[test]
fn integer_bounds_reject_overflow_and_keep_the_owner_permit() {
    let permit = Arc::new(());
    let weak = Arc::downgrade(&permit);
    let share = Share::checked(usize::MAX, 0, usize::MAX, 256, permit).unwrap();
    let funding = Funding::new(share, Duration::from_nanos(1)).unwrap();
    assert!(funding.admit(Identity::default(), [0; 6], usize::MAX, Duration::from_secs(1)).is_none());
    assert!(weak.upgrade().is_some());
    drop(funding);
    assert!(weak.upgrade().is_none());
    assert!(Funding::new(Share::checked(1 << 20, 0, 1 << 20, 16, Arc::new(())).unwrap(), Duration::ZERO).is_none());
}

#[test]
fn exported_frame_keeps_funding_after_handoff_destruction() {
    let handoff = Arc::new(Handoff::default());
    let id = Identity::default();
    handoff.set_tap_identity(id);
    let permit = Arc::new(());
    let weak = Arc::downgrade(&permit);
    handoff.fund_taps(Share::checked(1 << 20, 0, 1 << 20, 16, permit).unwrap(), Duration::from_secs(1)).unwrap();
    assert!(handoff.admit_taps(id, [0; 6], 1, Duration::from_secs(1)));
    let charge = handoff.reserve_plan(0, id).unwrap();
    let mut frame = crate::board::Frame::default();
    frame.basic.retain(charge);
    let retained = Arc::new(frame);
    let copy = std::hint::black_box(retained.as_ref().clone());
    drop(handoff);
    assert!(weak.upgrade().is_some());
    drop(retained);
    assert!(weak.upgrade().is_none());
    drop(copy);
}

#[test]
fn legacy_pending_read_and_consumed_reader_block_reassignment() {
    let handoff = Arc::new(Handoff::default());
    let share = || Share::checked(1 << 20, 0, 1 << 20, 16, Arc::new(())).unwrap();
    let pending = handoff.reserve_tap(1, 0, None).unwrap();
    assert!(handoff.fund_taps(share(), Duration::from_secs(1)).is_none());
    handoff
        .send_tap(
            Tap {
                frame: Arc::new(crate::board::Frame::default()),
                identity: None,
                projection: TapProjection::Unpainted,
                folds: None,
            },
            0,
            Some(&pending),
        )
        .unwrap();
    let TapRead::Ready((_, reader)) = handoff.take_tap(None, None, None, true) else { panic!("missing tap") };
    handoff.reset(1);
    drop(pending);
    assert!(handoff.fund_taps(share(), Duration::from_secs(1)).is_none());
    drop(reader);
    assert!(handoff.fund_taps(share(), Duration::from_secs(1)).is_some());
    assert!(handoff.reserve_tap(1, 1, None).is_none(), "legacy capacity cannot coexist with funding");
    assert!(handoff.fund_taps(share(), Duration::from_secs(1)).is_none());
}

#[test]
fn interval_wait_cancels_on_plan_retirement_without_waiting_for_the_clock() {
    let handoff = Arc::new(Handoff::default());
    let id = Identity::default();
    handoff.set_tap_identity(id);
    let cycle = Duration::from_secs(1);
    handoff.fund_taps(Share::checked(1 << 20, 0, 1 << 20, 16, Arc::new(())).unwrap(), cycle).unwrap();
    assert!(handoff.admit_taps(id, [0; 6], 1, cycle));
    let now = Instant::now();
    assert_eq!(handoff.tap_start(id, now), Some(Duration::ZERO));
    let charge = handoff.reserve_plan(0, id).unwrap();
    std::thread::scope(|threads| {
        let waiting = threads.spawn(|| handoff.wait_for_cycle(&charge, || now));
        handoff.wait_for_sender();
        handoff.retire_tap_plan();
        assert!(waiting.join().unwrap().is_none());
    });
}

#[test]
fn reset_returns_fifo_capacity_but_preserves_old_reader_funding() {
    let handoff = Arc::new(Handoff::default());
    let id = Identity::default();
    handoff.set_tap_identity(id);
    let cycle = Duration::from_secs(1);
    handoff.fund_taps(Share::checked(1 << 20, 0, 1 << 20, 8, Arc::new(())).unwrap(), cycle).unwrap();
    assert!(handoff.admit_taps(id, [0; 6], 1, cycle));
    let charge = handoff.reserve_plan(0, id).unwrap();
    let mut frame = crate::board::Frame::default();
    frame.basic.retain(charge.clone());
    let retained = Arc::new(frame);
    let tap = Tap { frame: retained.clone(), identity: Some(id), projection: TapProjection::Unpainted, folds: None };
    handoff.send_tap(tap, 0, Some(&charge)).unwrap();
    drop(charge);
    handoff.reset(1);
    assert_eq!(handoff.tap_count(), 0);
    assert_eq!(handoff.tap_drain(), 4);
    assert!(!handoff.admit_taps(id, [1; 6], 1, cycle));
    drop(retained);
    assert!(handoff.admit_taps(id, [1; 6], 1, cycle));
    assert!(handoff.reserve_plan(1, id).is_some());
}
