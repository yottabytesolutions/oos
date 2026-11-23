// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! Allocation guards of the batch read and decode (docs/fork/upstream-audit.md, P3 guard 1), in their own binary for
//! the counting global allocator. On the unit every heap block of 128 kB or more is a fresh mmap with page faults
//! (musl), so an allocation per read or per frame costs far more than on the host. Break it catches: a merge that
//! brings back `read_dma` or `deinterleave` + `volts` in the batch loop, or a decode that allocates scratch space.

use oos_hal::dho::bus::{DmaBuf, Recorder};
use oos_hal::dho::seq::{decode_volts, decode_volts_into, execute_bus, frame_stride, read_len, Step};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

/// Counts allocations (and reallocations) of the current thread: the tests of this binary run in parallel threads.
struct Counting;

thread_local! {
    static ALLOCS: Cell<usize> = const { Cell::new(0) };
}

fn bump() {
    // after the thread's locals are gone (thread exit) there is nothing to count
    let _ = ALLOCS.try_with(|n| n.set(n.get() + 1));
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        bump();
        System.alloc(l)
    }
    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        bump();
        System.alloc_zeroed(l)
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        bump();
        System.realloc(p, l, n)
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        System.dealloc(p, l)
    }
}

#[global_allocator]
static A: Counting = Counting;

/// Allocations of the current thread while `f` runs.
fn allocs<T>(f: impl FnOnce() -> T) -> (usize, T) {
    let before = ALLOCS.with(|n| n.get());
    let r = f();
    (ALLOCS.with(|n| n.get()) - before, r)
}

/// The C2H stream of the tests: hands out a fixed buffer, allocates nothing.
struct Mem<'a>(&'a [u8], usize);

impl std::io::Read for Mem<'_> {
    fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
        let k = b.len().min(self.0.len() - self.1);
        b[..k].copy_from_slice(&self.0[self.1..self.1 + k]);
        self.1 += k;
        Ok(k)
    }
}

fn bytes(n: usize) -> Vec<u8> {
    (0..n).map(|i| (i * 31 % 251) as u8).collect()
}

const CAL: [(f64, f64); 4] = [(1.0, 0.0), (0.5, 0.1), (2.0, -0.3), (1.0, 0.0)];

#[test]
fn dma_buf_reads_allocate_nothing_after_the_first() {
    let data = bytes(4_000_000);
    let mut dma = DmaBuf::new();
    dma.read(&mut Mem(&data, 0), data.len(), |_, _| {}).unwrap();
    for n in [data.len(), 2816 * 1420, 1000] {
        let (k, _) = allocs(|| {
            dma.read(&mut Mem(&data, 0), n, |_, _| {}).unwrap();
        });
        assert_eq!(k, 0, "read of {} bytes", n);
    }
}

const WARM_BYTE: u8 = 37;

struct RetainedRead<'a> {
    input: &'a [u8],
    expected: *const u8,
    calls: usize,
}

impl std::io::Read for RetainedRead<'_> {
    fn read(&mut self, dst: &mut [u8]) -> std::io::Result<usize> {
        self.calls += 1;
        assert_eq!(dst.as_ptr(), self.expected, "DMA must read directly into the retained buffer");
        assert_eq!(dst.len(), self.input.len(), "one request covers this DMA batch");
        assert!(dst.iter().all(|&byte| byte == WARM_BYTE), "a warmed read must leave the buffer untouched until DMA");
        dst.copy_from_slice(self.input);
        Ok(dst.len())
    }
}

#[test]
fn the_read_step_reuses_storage_without_zero_fill_or_allocation() {
    for (frame, frames) in [(2816, 1420), (2500, 1600), (200_000, 20), (4000, 1000), (8000, 500)] {
        let size = read_len(frames, frame);
        let data = bytes(size);
        let warm = vec![WARM_BYTE; size];
        let steps = [Step::Read { frames, frame }];
        let (mut bar, mut dma) = (Recorder::default(), DmaBuf::new());
        execute_bus(&steps, &mut bar, &mut Mem(&warm, 0), &mut dma, |_| {}).unwrap();
        let expected = dma.data().as_ptr();
        let mut read = RetainedRead { input: &data, expected, calls: 0 };
        let (allocations, result) = allocs(|| execute_bus(&steps, &mut bar, &mut read, &mut dma, |_| {}));
        result.unwrap();
        assert_eq!((allocations, read.calls), (0, 1), "frame {frame}");
        assert_eq!(dma.data().as_ptr(), expected, "unpadding must stay in the DMA buffer");
        let payload = data.chunks_exact(frame_stride(frame)).flat_map(|slot| &slot[..frame]);
        assert_eq!(dma.data().len(), frames * frame);
        assert!(dma.data().iter().eq(payload), "unpadding must preserve frame payloads");
        println!("DMA_READ_WORK frame={frame} frames={frames} bytes={size} reads=1 allocations=0");
    }
}

#[test]
fn decode_allocates_one_vector_per_enabled_channel() {
    for (mask, frame) in [(1u32, 2816usize), (0b0100, 200_000), (3, 4000), (0b1010, 4002), (0b0111, 8000), (0xf, 8000)]
    {
        let f = bytes(frame);
        decode_volts(&f, mask, CAL);
        let (k, out) = allocs(|| decode_volts(&f, mask, CAL));
        assert_eq!(k, mask.count_ones() as usize, "mask {:#x}", mask);
        assert_eq!(out.iter().filter(|v| !v.is_empty()).count(), mask.count_ones() as usize);
    }
}

#[test]
fn decode_into_kept_vectors_allocates_nothing_after_warm_up() {
    let mut out: [Vec<f32>; 4] = Default::default();
    for (mask, frame) in [(1u32, 2816usize), (1, 200_000), (3, 4000), (0xf, 8000)] {
        let f = bytes(frame);
        decode_volts_into(&f, mask, CAL, &mut out);
        for _ in 0..3 {
            let (k, _) = allocs(|| decode_volts_into(&f, mask, CAL, &mut out));
            assert_eq!(k, 0, "mask {:#x}, {} bytes", mask, frame);
        }
    }
}
