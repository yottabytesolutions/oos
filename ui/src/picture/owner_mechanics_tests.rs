// SPDX-License-Identifier: GPL-2.0-or-later
//! Real ownership counterexamples, not a production inbox or scheduling policy.
use super::*;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

struct Fixture {
    slot: Arc<Handoff>,
    window: DepositWindow,
    ph: Phosphor,
    origin: Instant,
    counters: Counters,
    edges: [u64; 4],
}
impl Fixture {
    fn new() -> Self {
        Self {
            slot: Arc::default(),
            window: Default::default(),
            ph: Phosphor::new(4, 4),
            origin: Instant::now(),
            counters: Default::default(),
            edges: [0; 4],
        }
    }
    fn at(&self, ms: u64) -> Instant {
        self.origin + Duration::from_millis(ms)
    }
    fn make(&self, records: u64, fed_ms: u64, row: usize) -> Deposit {
        self.make_at(records, self.at(fed_ms), row)
    }
    fn make_at(&self, records: u64, fed: Instant, row: usize) -> Deposit {
        let mut d = self.slot.acquire(1, (4, 4), Some(Identity::default()), [true, false, false, false]).unwrap();
        d.records = records;
        d.edges[0] = records;
        d.extent.include(fed);
        d.extent.add_hits(records);
        for band in &mut d.planes {
            band.test_span(row, (24 * records) as u16);
        }
        d
    }
    fn show(&mut self, mut d: Deposit) -> u64 {
        let accepted = accept_deposit(
            &mut d,
            Stats { edges: &mut self.edges, off: &mut (0.0, 0.0) },
            Some(Identity::default()),
            &self.counters,
        );
        self.window.push(d, &self.slot);
        accepted.frames
    }
    fn three_shown(&mut self) {
        for (row, records) in [100, 1, 1].into_iter().enumerate() {
            let deposit = self.make(records, row as u64, row);
            self.show(deposit);
        }
        assert_eq!(self.window.rebuild(&mut self.ph), 102);
        assert_eq!(self.window.deposits.len(), 3);
        assert!((self.window.target - 81.19).abs() < 1e-9);
    }
    fn intake(&self, d: Deposit) -> Deposit {
        assert!(self.slot.put(d).is_ok());
        let mut claim = self.slot.claim().unwrap();
        let owned = claim.deposit.take().unwrap();
        drop(claim);
        owned
    }
    fn pixels(&self) -> Vec<u16> {
        self.ph.maps()[0].to_vec()
    }
}

#[test]
fn independent_intake_is_uncredited_but_per_entry_push_loses_picture_group() {
    let mut f = Fixture::new();
    let before = f.counters.records_text();
    let first = f.intake(f.make(32, 0, 0));
    let second = f.intake(f.make(32, 1, 1));
    assert_eq!((f.window.target, f.window.deposits.len(), f.edges), (0.0, 0, [0; 4]));
    assert_eq!(f.counters.records_text(), before);
    assert!(f.pixels().iter().all(|&x| x == 0));
    let accepted = f.show(first) + f.show(second);
    assert_eq!(accepted, 64, "both entries would be credited by the existing accept path");
    assert_eq!(f.edges[0], 64);
    assert_eq!(f.window.target, 32.0, "naive push updates density per entry, not64-record picture group");
    assert_eq!(f.window.rebuild(&mut f.ph), 32, "the first new entry was evicted before final fold");
    assert!(f.ph.maps()[0][..4].iter().all(|&x| x == 0));
    assert!(f.ph.maps()[0][4..8].iter().all(|&x| x == 32 * 24));
    f.window.reset(&f.slot);
    assert_eq!(f.window.target, 0.0);
}

#[test]
fn three_live_window_sets_plus_intake_and_open_block_a_sixth_across_reset() {
    let mut f = Fixture::new();
    f.three_shown();
    let owned = f.intake(f.make(32, 52, 3));
    let open = f.make_at(32, f.at(66) + Duration::from_micros(700), 0);
    assert_eq!(f.slot.backing_count(), 5);
    let target = f.window.target;
    let pixels = f.pixels();
    let edges = f.edges;
    assert!(!f.window.remove_invalid(&f.slot, |d| d.identity == Some(Identity::default())));
    assert_eq!((f.window.target, f.edges), (target, edges));
    assert_eq!(f.pixels(), pixels);
    assert!(f.slot.reset(2).is_none());
    assert!(f.slot.acquire(1, (4, 4), Some(Identity::default()), [true, false, false, false]).is_none());
    std::thread::scope(|threads| {
        let slot = f.slot.clone();
        let waiting =
            threads.spawn(move || slot.acquire(2, (4, 4), Some(Identity::default()), [true, false, false, false]));
        f.slot.wait_for_backing();
        let count = f.slot.backing_count();
        f.slot.close();
        assert_eq!(count, 5);
        assert!(waiting.join().unwrap().is_none());
    });
    assert_eq!(f.window.deposits.len(), 3, "live shown retention is not automatically disposable");
    drop((owned, open));
    assert_eq!(f.slot.backing_count(), 3);
    f.window.reset(&f.slot);
}

#[test]
fn invalid_retained_identity_releases_a_backing_without_changing_the_composed_map() {
    let mut f = Fixture::new();
    f.three_shown();
    let owned = f.intake(f.make(32, 52, 3));
    let open = f.make(32, 66, 0);
    let pixels = f.pixels();
    let target = f.window.target;
    let edges = f.edges;
    let counters = f.counters.records_text();
    std::thread::scope(|threads| {
        let slot = f.slot.clone();
        let waiting =
            threads.spawn(move || slot.acquire(1, (4, 4), Some(Identity::default()), [true, false, false, false]));
        f.slot.wait_for_backing();
        let dirty = f.window.remove_invalid(&f.slot, |d| d.identity.is_none());
        if !dirty {
            f.slot.close();
        }
        let next_open = waiting.join().unwrap().unwrap();
        assert!(dirty);
        assert_eq!(f.slot.backing_count(), 5, "recycle the existing backing");
        assert!(f.window.is_empty());
        assert_eq!((f.window.target, f.edges), (target, edges));
        assert_eq!(f.counters.records_text(), counters);
        assert_eq!(f.pixels(), pixels);
        assert_eq!(f.window.rebuild(&mut f.ph), 0);
        drop(next_open);
    });
    drop((owned, open));
}
#[test]
fn identity_invalidation_rebuilds_the_picture_without_counting_retained_records_again() {
    let mut f = Fixture::new();
    let d = f.make(32, 0, 0);
    f.show(d);
    assert_eq!(f.window.rebuild(&mut f.ph), 32);
    assert_eq!(f.window.target, 32.0);
    assert_eq!(f.edges[0], 32);
    let pipe = crate::drawpipe::Pipe::start(0, 0, Box::leak(Box::default()));
    let accepted = pipe.accept_into(
        &mut f.ph,
        &mut true,
        Stats { edges: &mut f.edges, off: &mut (0.0, 0.0) },
        Some(&mut f.window),
    );
    assert_eq!(accepted.frames, 0);
    assert_eq!(accepted.window_frames, Some(0));
    assert!(f.window.is_empty());
    assert_eq!(f.window.target, 28.8, "one normal composition idle decay, none during intake");
    assert!(f.pixels().iter().all(|&x| x == 0));
    assert_eq!(f.edges[0], 32);
}
