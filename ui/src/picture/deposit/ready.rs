// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    OnceLock,
};

pub(crate) struct Claim<'a> {
    pub(crate) deposit: Option<Deposit>,
    slot: &'a Handoff,
}

impl<'a> Claim<'a> {
    pub(super) fn new(slot: &'a Handoff, deposit: Deposit) -> Self {
        Self { deposit: Some(deposit), slot }
    }
}

impl Drop for Claim<'_> {
    fn drop(&mut self) {
        oos_hal::sync::lock(&self.slot.state).claimed = false;
        self.slot.space.notify_all();
    }
}

#[derive(Default)]
pub(super) struct Signal {
    pub(super) enabled: AtomicBool,
    notified: AtomicBool,
    wake: OnceLock<Arc<crate::system::intake::Wake>>,
}
impl Signal {
    pub(super) fn enable(&self, active: bool, wake: Option<Arc<crate::system::intake::Wake>>) -> bool {
        if let Some(wake) = wake.filter(|_| active) {
            let _ = self.wake.set(wake);
        }
        let enabled = active && self.wake.get().is_some();
        // Only the render owner changes the enable state.
        if self.enabled.load(Ordering::Acquire) == enabled {
            return false;
        }
        if enabled {
            self.rearm();
            self.enabled.store(true, Ordering::Release);
        } else {
            self.enabled.store(false, Ordering::Release);
            self.rearm();
        }
        enabled
    }
    pub(super) fn rearm(&self) {
        self.notified.store(false, Ordering::Release);
    }
    pub(super) fn published(&self) {
        if self.enabled.load(Ordering::Acquire) && !self.notified.swap(true, Ordering::AcqRel) {
            if let Some(wake) = self.wake.get() {
                wake.notify();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn deposit(slot: &Handoff, gen: u64) -> Deposit {
        let mut d = slot.acquire(gen, (4, 4), None, [true, false, false, false]).unwrap();
        d.records = 32;
        d.extent.include(Instant::now());
        d
    }
    #[test]
    fn repeated_disable_leaves_notification_state_alone() {
        let signal = Signal::default();
        let wake = Arc::new(crate::system::intake::Wake::new().unwrap());
        assert!(signal.enable(true, Some(wake)));
        signal.published();
        assert!(signal.notified.load(Ordering::Relaxed));
        assert!(!signal.enable(false, None));
        assert!(!signal.notified.load(Ordering::Relaxed));

        // A publication can finish coalescing a notification after observing the old enable state.
        signal.notified.store(true, Ordering::Relaxed);
        assert!(!signal.enable(false, None));
        assert!(signal.notified.load(Ordering::Relaxed));
    }
    #[test]
    fn coalesced_real_pending_transition_and_drain_reset_rearm_cannot_lose_ready() {
        let slot = Handoff::default();
        let wake = Arc::new(crate::system::intake::Wake::new().unwrap());
        slot.owner_enable(true, Some(wake.clone()));
        assert!(!wake.wait(0));
        assert!(slot.put(deposit(&slot, 1)).is_ok());
        slot.owner_enable(true, Some(wake.clone()));
        let receiver = wake.receiver().unwrap();
        assert_eq!(receiver.recv(&mut [0]).unwrap(), 1);
        assert_eq!(
            receiver.recv(&mut [0]).unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock,
            "no duplicate activation wake"
        );
        let old = slot.reset(2).unwrap();
        slot.recycle(old);
        wake.drain();
        slot.owner_rearm();
        assert!(slot.put(deposit(&slot, 2)).is_ok());
        assert!(wake.wait(0));
        // This is the real order: drain shared Wake BEFORE checking ownership, never after.
        wake.drain();
        slot.owner_rearm();
        let mut claim = slot.claim().unwrap();
        let owned = claim.deposit.take().unwrap();
        drop(claim);
        assert!(!wake.wait(0));
        assert!(slot.put(deposit(&slot, 2)).is_ok());
        assert!(wake.wait(0));
        let mut next = slot.claim().unwrap();
        let second = next.deposit.take().unwrap();
        drop(next);
        wake.drain();
        slot.owner_rearm();
        assert!(slot.claim().is_none());
        assert!(!wake.wait(0));
        drop((owned, second));
        slot.close();
    }
}
