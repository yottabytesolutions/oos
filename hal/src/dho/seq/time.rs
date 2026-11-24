// SPDX-License-Identifier: GPL-2.0-or-later
//! Acquisition clock: real waits on the unit, explicitly advanced time in host fixtures.
pub use super::execute::{acquire_batch_observed, BatchPhase};
use std::time::{Duration, Instant};

pub trait Clock {
    fn now(&self) -> Instant;
    fn wait(&self, duration: Duration);
}
#[derive(Debug)]
pub struct Real;
impl Clock for Real {
    fn now(&self) -> Instant {
        Instant::now()
    }
    fn wait(&self, duration: Duration) {
        std::thread::sleep(duration);
    }
}

pub(super) const STOP_POLL: Duration = Duration::from_millis(10);

/// Fill budget and the end of arm writes when the caller armed ahead.
#[derive(Clone, Copy, Debug)]
pub struct BatchWait {
    fill: Duration,
    armed_at: Option<Instant>,
    pub(super) first: u32,
}

impl BatchWait {
    pub fn new(fill_ms: u64, armed_at: Option<Instant>) -> Self {
        Self { fill: Duration::from_millis(fill_ms), armed_at, first: 0 }
    }

    /// Apply the same ring start to arm, fill count and playback.
    pub fn from_frame(self, first: u32) -> Self {
        Self { first, ..self }
    }

    pub(super) fn fill(self) -> Duration {
        self.fill
    }

    pub(super) fn armed_at(self) -> Option<Instant> {
        self.armed_at
    }
}

pub(super) fn record_time(points: u32, rate: u64) -> Duration {
    Duration::from_nanos(u64::from(points) * 1_000_000_000 / rate.max(1))
}

pub(super) fn fill_count(
    bar: &mut impl crate::dho::bus::Bus,
    (nf, p): (u32, &super::hz::Params),
    (wait, started, estimate): (BatchWait, Instant, bool),
    clock: &impl Clock,
    (idle, stop): (&mut impl FnMut() -> bool, &impl Fn() -> bool),
) -> u32 {
    let record = if estimate { record_time(p.memsel, p.rate) } else { Duration::ZERO };
    let step = Duration::from_micros(200);
    let mut next_stop = started + STOP_POLL;
    loop {
        let count = bar.rd(0x4010).wrapping_add(1).saturating_sub(wait.first).clamp(1, nf);
        let now = clock.now();
        let left = wait.fill().saturating_sub(now.saturating_duration_since(started));
        if count == nf || left.is_zero() {
            return count;
        }
        if now >= next_stop {
            if stop() {
                return count;
            }
            next_stop = now + STOP_POLL;
        }
        if !idle() {
            let remaining = record.saturating_mul(nf - count);
            let delay = remaining.clamp(step, STOP_POLL);
            clock.wait(delay.min(left).min(next_stop.saturating_duration_since(now)));
        }
    }
}
