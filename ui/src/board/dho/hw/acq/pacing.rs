// SPDX-License-Identifier: GPL-2.0-or-later

#[cfg(target_os = "linux")]
use crate::pacing;
use crate::pacing::Fill;
use std::time::{Duration, Instant};

pub(super) trait PacingPolicy {
    fn picture_period(&self) -> Duration;
    fn fill_ms(&self, fill: &mut Fill, now: Instant) -> u64;
}

#[cfg(target_os = "linux")]
pub(super) struct DisplayPacing;

#[cfg(target_os = "linux")]
impl PacingPolicy for DisplayPacing {
    fn picture_period(&self) -> Duration {
        pacing::picture_period()
    }

    fn fill_ms(&self, fill: &mut Fill, now: Instant) -> u64 {
        fill.plan(now, pacing::next_step_after, pacing::ready_ms())
    }
}
