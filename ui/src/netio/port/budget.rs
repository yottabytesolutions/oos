// SPDX-License-Identifier: GPL-2.0-or-later
//! Reservations cover assembled, parked, queued and IO-held output without charging moves twice.
use super::{Wake, IMPORT_BYTES};
use std::sync::Arc;
use tokio::sync::{OwnedSemaphorePermit, Semaphore, SemaphorePermit};

pub struct Budget {
    available: Arc<Semaphore>,
    limit: usize,
    wake: Arc<Wake>,
}
impl Budget {
    #[cfg(test)]
    pub fn new(wake: Arc<Wake>) -> Arc<Self> {
        Self::bounded(wake, super::RESPONSE_BYTES)
    }
    pub(super) fn bounded(wake: Arc<Wake>, limit: usize) -> Arc<Self> {
        Arc::new(Self { available: Arc::new(Semaphore::new(limit)), limit, wake })
    }
    pub fn reserve(self: &Arc<Self>, bytes: usize) -> Option<Reservation> {
        let permit = self.available.clone().try_acquire_many_owned(bytes.try_into().ok()?).ok()?;
        Some(Reservation { permit, wake: self.wake.clone() })
    }
    pub fn reserve_available(self: &Arc<Self>) -> Option<Reservation> {
        loop {
            let available = self.available.available_permits();
            if available == 0 {
                return None;
            }
            if let Some(reservation) = self.reserve(available) {
                return Some(reservation);
            }
        }
    }
    pub fn used(&self) -> usize {
        self.limit - self.available.available_permits()
    }
}
pub struct Reservation {
    permit: OwnedSemaphorePermit,
    wake: Arc<Wake>,
}
impl Reservation {
    pub fn capacity(&self) -> usize {
        self.permit.num_permits()
    }
    pub fn retain(&mut self, bytes: usize) {
        assert!(bytes <= self.capacity(), "response allocation exceeds its reservation");
        drop(self.permit.split(self.capacity() - bytes));
        self.wake.notify();
    }
}
impl Drop for Reservation {
    fn drop(&mut self) {
        // Release before notifying the frame owner to retry a parked reservation.
        drop(self.permit.split(self.capacity()));
        self.wake.notify();
    }
}

pub struct LargeBodySlot(Semaphore);
impl Default for LargeBodySlot {
    fn default() -> Self {
        Self(Semaphore::new(1))
    }
}
impl LargeBodySlot {
    pub(crate) fn admit(&self, bytes: usize) -> Option<LargeBodyPermit<'_>> {
        if bytes > IMPORT_BYTES {
            return None;
        }
        self.0.try_acquire().ok().map(|permit| LargeBodyPermit { _permit: permit })
    }
    #[cfg(test)]
    pub(crate) fn occupied(&self) -> bool {
        self.0.available_permits() == 0
    }
}
pub struct LargeBodyPermit<'a> {
    _permit: SemaphorePermit<'a>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn available_reservation_takes_remaining_capacity_and_releases_it() {
        let budget = Budget::bounded(Arc::new(Wake::new().unwrap()), 8);
        let held = budget.reserve(3).unwrap();
        let rest = budget.reserve_available().unwrap();
        assert_eq!(rest.capacity(), 5);
        assert_eq!(budget.used(), 8);
        assert!(budget.reserve_available().is_none());
        drop(held);
        let released = budget.reserve_available().unwrap();
        assert_eq!(released.capacity(), 3);
        drop((rest, released));
        assert_eq!(budget.reserve_available().unwrap().capacity(), 8);
    }
}
