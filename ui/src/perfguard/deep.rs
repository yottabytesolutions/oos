// SPDX-License-Identifier: GPL-2.0-or-later

//! Bounded process diagnostics. Independent metric locks never nest with each other;
//! Shared is released before its wait/hold samples are recorded. Reporting sorts copies.

use super::{nanos, ring::Ring};
use oos_hal::sync::lock;
use std::ops::{Deref, DerefMut};
use std::sync::{Mutex, MutexGuard};
use std::time::Instant;

mod polls;
pub(crate) use polls::Poll;

#[derive(Debug, Default)]
struct LockTiming {
    wait: Ring,
    held: Ring,
    total_wait: u64,
}
#[derive(Debug, Default)]
struct CopyTiming {
    time: Ring,
}

/// 64 KiB budget per process, with at least 8 KiB spare; no record data or per-observation allocation after mutex warmup.
#[derive(Debug, Default)]
pub(crate) struct Diagnostics {
    polls: [Mutex<Ring>; Poll::ALL.len()],
    locks: [Mutex<LockTiming>; 2],
    copies: Mutex<CopyTiming>,
}
impl Diagnostics {
    pub(crate) fn poll(&self, poll: Poll, elapsed: std::time::Duration) {
        lock(&self.polls[poll as usize]).add(nanos(elapsed));
    }
    fn locked(&self, side: usize, wait: u64, held: u64) {
        let mut rings = lock(&self.locks[side]);
        rings.wait.add(wait);
        rings.total_wait = rings.total_wait.wrapping_add(wait);
        rings.held.add(held);
    }
    #[cfg(test)]
    pub(super) fn locked_for_test(&self, wait: u64) {
        self.locked(0, wait, 0);
    }
    #[cfg(test)]
    pub(crate) fn hold_copies_for_test(&self) -> impl Drop + '_ {
        lock(&self.copies)
    }
    pub(crate) fn ui_wait(&self) -> u64 {
        lock(&self.locks[0]).total_wait
    }
    pub(crate) fn text(&self) -> String {
        let mut text = String::new();
        for (poll, ring) in Poll::ALL.iter().zip(&self.polls) {
            let sample = { lock(ring).clone() };
            text.push_str(&sample.text(&format!("poll.{}", poll.label()), "ns"));
        }
        for (side, rings) in ["ui", "acq"].iter().zip(&self.locks) {
            let (wait, held) = {
                let rings = lock(rings);
                (rings.wait.clone(), rings.held.clone())
            };
            text.push_str(&wait.text(&format!("lock.{side}.wait"), "ns"));
            text.push_str(&held.text(&format!("lock.{side}.held"), "ns"));
        }
        let time = { lock(&self.copies).time.clone() };
        text.push_str(&time.text("producer.clone", "ns"));
        text
    }
}

pub(crate) fn counters() -> &'static Diagnostics {
    crate::drawpipe::counters().acquisition.diagnostics()
}

/// Time publication handle cloning.
pub(crate) fn clone_frame(
    frame: &std::sync::Arc<crate::board::Frame>,
) -> (std::sync::Arc<crate::board::Frame>, CopySample) {
    let start = Instant::now();
    let copy = frame.clone();
    let elapsed = nanos(start.elapsed());
    (copy, CopySample { elapsed })
}

#[must_use = "record the copy sample after releasing Shared"]
pub(crate) struct CopySample {
    elapsed: u64,
}
impl CopySample {
    pub(crate) fn record(self) {
        let mut rings = lock(&counters().copies);
        rings.time.add(self.elapsed);
    }
}

pub(crate) struct Guard<'a, T> {
    guard: Option<MutexGuard<'a, T>>,
    diagnostics: &'a Diagnostics,
    side: usize,
    wait: u64,
    acquired: Instant,
}
impl<T> Deref for Guard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.guard.as_deref().expect("the guard exists until Drop releases it")
    }
}
impl<T> DerefMut for Guard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.guard.as_deref_mut().expect("the guard exists until Drop releases it")
    }
}
impl<T> Drop for Guard<'_, T> {
    fn drop(&mut self) {
        self.record_at(Instant::now());
    }
}

impl<T> Guard<'_, T> {
    fn record_at(&mut self, now: Instant) {
        if let Some(guard) = self.guard.take() {
            let held = nanos(now.saturating_duration_since(self.acquired));
            drop(guard);
            self.diagnostics.locked(self.side, self.wait, held);
        }
    }
}

fn timed_lock<'a, T>(mutex: &'a Mutex<T>, diagnostics: &'a Diagnostics, side: usize) -> Guard<'a, T> {
    timed_lock_with(mutex, diagnostics, side, Instant::now)
}

fn timed_lock_with<'a, T>(
    mutex: &'a Mutex<T>,
    diagnostics: &'a Diagnostics,
    side: usize,
    mut now: impl FnMut() -> Instant,
) -> Guard<'a, T> {
    let started = now();
    let guard = lock(mutex);
    let acquired = now();
    Guard { guard: Some(guard), diagnostics, side, wait: nanos(acquired - started), acquired }
}

pub(crate) fn ui_lock<T>(mutex: &Mutex<T>) -> Guard<'_, T> {
    timed_lock(mutex, counters(), 0)
}
#[cfg(any(target_os = "linux", test))]
pub(crate) fn acq_lock<T>(mutex: &Mutex<T>) -> Guard<'_, T> {
    timed_lock(mutex, counters(), 1)
}

#[cfg(test)]
mod tests;
