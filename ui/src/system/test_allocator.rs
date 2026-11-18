// SPDX-License-Identifier: GPL-2.0-or-later

use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};

thread_local! {
    // The calling thread's own share: requested bytes minus released bytes, and its allocation count. Exact
    // assertions read these; the process-wide counters also see the test harness thread, which allocates its
    // bookkeeping while the test thread already runs.
    static OWN_PEAK: Cell<isize> = const { Cell::new(0) };
    static OWN_LIVE: Cell<isize> = const { Cell::new(0) };
    static OWN_ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}

/// Counts allocations and reallocations made by the calling thread, including freed transients.
pub fn count_own<T>(f: impl FnOnce() -> T) -> (usize, T) {
    let before = OWN_ALLOCATIONS.get();
    let result = f();
    (OWN_ALLOCATIONS.get() - before, result)
}

/// Peak live bytes above entry on the calling thread, including freed transients.
pub fn own_high_water<T>(f: impl FnOnce() -> T) -> (usize, T) {
    let baseline = OWN_LIVE.get();
    let previous = OWN_PEAK.replace(baseline);
    let result = f();
    let peak = OWN_PEAK.replace(previous.max(OWN_PEAK.get()));
    ((peak - baseline) as usize, result)
}

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
    /// Like `count`, but only the allocations of the calling thread.
    pub fn count_own<T>(&self, f: impl FnOnce() -> T) -> (usize, T) {
        count_own(f)
    }
    pub fn allocated(&self, bytes: usize) {
        self.large.fetch_add(usize::from(bytes >= self.large_min.load(SeqCst)), SeqCst);
        self.added(bytes);
        self.counted();
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
        self.counted();
    }
    pub fn live(&self) -> usize {
        self.live.load(SeqCst)
    }
    /// Bytes the calling thread requested minus the bytes it released, of any thread's allocations.
    pub fn own_live(&self) -> isize {
        OWN_LIVE.get()
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
    fn counted(&self) {
        self.allocations.fetch_add(1, SeqCst);
        OWN_ALLOCATIONS.set(OWN_ALLOCATIONS.get().wrapping_add(1));
    }
    fn added(&self, bytes: usize) {
        let live = self.live.fetch_add(bytes, SeqCst) + bytes;
        self.peak.fetch_max(live, SeqCst);
        OWN_LIVE.set(OWN_LIVE.get().wrapping_add_unsigned(bytes));
        OWN_PEAK.set(OWN_PEAK.get().max(OWN_LIVE.get()));
    }
    fn removed(&self, bytes: usize) {
        self.live.fetch_sub(bytes, SeqCst);
        OWN_LIVE.set(OWN_LIVE.get().wrapping_sub_unsigned(bytes));
    }
}

use std::alloc::{GlobalAlloc, Layout, System};

// SAFETY: every allocation and layout is delegated unchanged to System. Atomic counters never allocate.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the caller supplies GlobalAlloc's valid layout; System owns the returned allocation.
        let p = unsafe { System.alloc(layout) };
        if !p.is_null() {
            self.allocated(layout.size());
        }
        p
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the valid layout is passed unchanged to System, preserving its zero-initialization contract.
        let p = unsafe { System.alloc_zeroed(layout) };
        if !p.is_null() {
            self.allocated(layout.size());
        }
        p
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: the caller provides the System allocation and its original layout.
        unsafe { System.dealloc(ptr, layout) };
        self.freed(layout.size());
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        // SAFETY: the caller provides the live System allocation, original layout and valid new size.
        let p = unsafe { System.realloc(ptr, layout, size) };
        if !p.is_null() {
            self.resized(layout.size(), size);
        }
        p
    }
}
