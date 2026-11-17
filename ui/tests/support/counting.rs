// SPDX-License-Identifier: GPL-2.0-or-later

use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};

pub struct Counting {
    live: AtomicUsize,
    peak: AtomicUsize,
    allocations: AtomicUsize,
    frees: AtomicUsize,
    large: AtomicUsize,
    large_min: AtomicUsize,
}

impl Default for Counting {
    fn default() -> Self {
        Self::new()
    }
}

impl Counting {
    pub const fn new() -> Self {
        Self {
            live: AtomicUsize::new(0),
            peak: AtomicUsize::new(0),
            allocations: AtomicUsize::new(0),
            frees: AtomicUsize::new(0),
            large: AtomicUsize::new(0),
            large_min: AtomicUsize::new(2_000_000),
        }
    }
    /// Counts transient allocations and reallocations too, not just retained bytes.
    pub fn count<T>(&self, f: impl FnOnce() -> T) -> (usize, T) {
        let before = self.allocations();
        let result = f();
        (self.allocations() - before, result)
    }
    pub fn allocated(&self, bytes: usize) {
        self.large.fetch_add(usize::from(bytes >= self.large_min.load(SeqCst)), SeqCst);
        self.added(bytes);
        self.allocations.fetch_add(1, SeqCst);
    }
    pub fn freed(&self, bytes: usize) {
        self.removed(bytes);
        self.frees.fetch_add(1, SeqCst);
    }
    pub fn resized(&self, old: usize, new: usize) {
        self.large.fetch_add(usize::from(new >= self.large_min.load(SeqCst)), SeqCst);
        if new >= old {
            self.added(new - old);
        } else {
            self.removed(old - new);
        }
        self.allocations.fetch_add(1, SeqCst);
    }
    pub fn live(&self) -> usize {
        self.live.load(SeqCst)
    }
    pub fn peak(&self) -> usize {
        self.peak.load(SeqCst)
    }
    pub fn set_large_min(&self, bytes: usize) {
        self.large_min.store(bytes, SeqCst);
    }
    pub fn large_allocations(&self) -> usize {
        self.large.load(SeqCst)
    }
    pub fn allocations(&self) -> usize {
        self.allocations.load(SeqCst)
    }
    pub fn frees(&self) -> usize {
        self.frees.load(SeqCst)
    }
    fn added(&self, bytes: usize) {
        let live = self.live.fetch_add(bytes, SeqCst) + bytes;
        self.peak.fetch_max(live, SeqCst);
    }
    fn removed(&self, bytes: usize) {
        self.live.fetch_sub(bytes, SeqCst);
    }
}
