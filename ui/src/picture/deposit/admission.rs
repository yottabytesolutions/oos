// SPDX-License-Identifier: GPL-2.0-or-later
//! Checked input from the application memory owner. No device default grant exists.

use super::tap::{Entry, Unpublished};
use oos_core::change::Identity;
use oos_hal::sync::lock;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Move-only partition of an audited application envelope. The permit must keep the
/// parent allocation charged until the last frame/result releases this partition.
pub(crate) struct Share {
    bytes: usize,
    entries: usize,
    _permit: Arc<dyn std::any::Any + Send + Sync>,
}

impl Share {
    pub(crate) fn checked(
        envelope: usize,
        other_owners: usize,
        bytes: usize,
        entries: usize,
        permit: Arc<dyn std::any::Any + Send + Sync>,
    ) -> Option<Self> {
        (bytes > 0 && entries > 0 && other_owners.checked_add(bytes)? <= envelope).then_some(Self {
            bytes,
            entries,
            _permit: permit,
        })
    }
}

pub(crate) struct Funding {
    share: Share,
    used: Mutex<(usize, usize)>,
    interval: Duration,
    last: Mutex<Option<(Instant, Duration)>>,
}

impl Funding {
    pub(crate) fn new(share: Share, interval: Duration) -> Option<Arc<Self>> {
        if share.entries < 8 || interval.is_zero() || interval > Duration::from_secs(1) {
            return None;
        }
        let overhead = share
            .entries
            .checked_mul(
                std::mem::size_of::<Entry>()
                    + std::mem::size_of::<std::sync::Weak<Plan>>()
                    + std::mem::size_of::<Unpublished>(),
            )?
            .checked_add(std::mem::size_of::<Self>() + 2 * std::mem::size_of::<usize>())?;
        (overhead <= share.bytes)
            .then(|| Arc::new(Self { share, used: Mutex::new((overhead, 0)), interval, last: Mutex::new(None) }))
    }

    pub(crate) fn entries(&self) -> usize {
        self.share.entries
    }

    #[cfg(any(target_os = "linux", test))]
    pub(crate) fn admit(
        self: &Arc<Self>,
        identity: Identity,
        format: [u64; 6],
        samples: usize,
        limit: Duration,
    ) -> Option<Arc<Plan>> {
        const PERIOD_NS: u64 = 33_333_334;
        let record = samples
            .checked_mul(std::mem::size_of::<f32>())?
            .checked_add(std::mem::size_of::<crate::board::Frame>())?
            .checked_add(std::mem::size_of::<super::TapReservation>() + 4 * std::mem::size_of::<usize>())?;
        let overhead = std::mem::size_of::<Plan>() + 2 * std::mem::size_of::<usize>();
        let mut used = lock(&self.used);
        let slots = self.share.bytes.checked_sub(used.0)?.checked_sub(overhead)? / record;
        let available = (self.share.entries - used.1).min(slots.checked_sub(2)?);
        let cycles = u64::try_from(available / 2).ok()?.checked_sub(3)?;
        if cycles == 0 {
            return None;
        }
        let interval = self.interval.max(Duration::from_nanos(PERIOD_NS.div_ceil(cycles)));
        if interval > limit {
            return None;
        }
        let b = usize::try_from(u128::from(PERIOD_NS).div_ceil(interval.as_nanos())).ok()? + 3;
        let fifo = 2 * b;
        let bytes = (fifo + 2) * record + overhead;
        used.0 += bytes;
        used.1 += fifo;
        Some(Arc::new(Plan {
            funding: self.clone(),
            identity,
            format,
            samples,
            b,
            fifo,
            bytes,
            interval,
            occupancy: Mutex::new((0, 0)),
        }))
    }
}

pub(crate) struct Plan {
    funding: Arc<Funding>,
    identity: Identity,
    format: [u64; 6],
    samples: usize,
    b: usize,
    fifo: usize,
    bytes: usize,
    interval: Duration,
    // All live frames and reserved/queued FIFO entries, respectively.
    occupancy: Mutex<(usize, usize)>,
}

impl Plan {
    #[cfg(any(target_os = "linux", test))]
    pub(crate) fn format(&self, id: Identity, format: [u64; 6]) -> bool {
        self.identity(id) && self.format == format
    }
    pub(crate) fn identity(&self, id: Identity) -> bool {
        crate::drawpipe::same_pair(Some(self.identity), Some(id))
    }
    #[cfg(any(target_os = "linux", test))]
    pub(crate) fn samples(&self) -> usize {
        self.samples
    }
    pub(crate) fn drain(&self) -> usize {
        self.b
    }
    #[cfg(any(target_os = "linux", test))]
    pub(crate) fn reserve(&self) {
        let mut n = lock(&self.occupancy);
        n.0 += 1;
        n.1 += 1;
    }
    pub(crate) fn fits(&self) -> bool {
        let n = lock(&self.occupancy);
        n.0 < self.fifo + 2 && n.1 < self.fifo
    }
    pub(crate) fn release(&self, queued: bool) {
        let mut n = lock(&self.occupancy);
        n.0 -= 1;
        n.1 -= usize::from(queued);
    }
    pub(crate) fn dequeued(&self) {
        lock(&self.occupancy).1 -= 1;
    }
    /// Enforce both adjacent plans' bounds across a format change.
    #[cfg(any(target_os = "linux", test))]
    pub(crate) fn start(&self, now: Instant) -> Duration {
        let mut last = lock(&self.funding.last);
        if let Some((previous, interval)) = *last {
            let wait = interval.max(self.interval).saturating_sub(now.saturating_duration_since(previous));
            if !wait.is_zero() {
                return wait;
            }
        }
        *last = Some((now, self.interval));
        Duration::ZERO
    }
}

impl Drop for Plan {
    fn drop(&mut self) {
        let mut used = lock(&self.funding.used);
        used.0 -= self.bytes;
        used.1 -= self.fifo;
    }
}

#[cfg(test)]
mod tests;
