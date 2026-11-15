// SPDX-License-Identifier: GPL-2.0-or-later

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};

#[derive(Debug)]
pub struct Counting {
    pub live: AtomicUsize,
    pub peak: AtomicUsize,
    pub allocations: AtomicUsize,
    pub frees: AtomicUsize,
}

impl Counting {
    pub const fn new() -> Self {
        Self {
            live: AtomicUsize::new(0),
            peak: AtomicUsize::new(0),
            allocations: AtomicUsize::new(0),
            frees: AtomicUsize::new(0),
        }
    }

    /// Counts transient allocations and reallocations too, not just retained bytes.
    pub fn count<T>(&self, f: impl FnOnce() -> T) -> (usize, T) {
        let before = self.allocations.load(SeqCst);
        let result = f();
        (self.allocations.load(SeqCst) - before, result)
    }

    fn added(&self, bytes: usize) {
        let live = self.live.fetch_add(bytes, SeqCst) + bytes;
        self.peak.fetch_max(live, SeqCst);
    }
    fn removed(&self, bytes: usize) {
        self.live.fetch_sub(bytes, SeqCst);
    }
}

// SAFETY: every allocation and layout is delegated unchanged to System. Atomic counters never allocate.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the caller supplies GlobalAlloc's valid layout; System owns the returned allocation.
        let p = unsafe { System.alloc(layout) };
        if !p.is_null() {
            self.added(layout.size());
            self.allocations.fetch_add(1, SeqCst);
        }
        p
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the valid layout is passed unchanged to System, preserving its zero-initialization contract.
        let p = unsafe { System.alloc_zeroed(layout) };
        if !p.is_null() {
            self.added(layout.size());
            self.allocations.fetch_add(1, SeqCst);
        }
        p
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: the caller provides the System allocation and its original layout.
        unsafe { System.dealloc(ptr, layout) };
        self.removed(layout.size());
        self.frees.fetch_add(1, SeqCst);
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        // SAFETY: the caller provides the live System allocation, original layout and valid new size.
        let p = unsafe { System.realloc(ptr, layout, size) };
        if !p.is_null() {
            if size >= layout.size() {
                self.added(size - layout.size());
            } else {
                self.removed(layout.size() - size);
            }
            self.allocations.fetch_add(1, SeqCst);
        }
        p
    }
}
impl Default for Counting {
    fn default() -> Self {
        Self::new()
    }
}
