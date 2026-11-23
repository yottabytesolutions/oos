// SPDX-License-Identifier: GPL-2.0-or-later

use super::outputs::Outputs;
use crate::pacing::{Flip, Picture};
use std::time::{Duration, Instant};

/// Convert a DRM or helper completion timestamp to the UI clock.
pub(super) fn instant(timestamp: Duration) -> Instant {
    let mut clock = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    // SAFETY: clock points to writable storage for one CLOCK_MONOTONIC timestamp.
    let status = unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut clock) };
    let now = Instant::now();
    if status == 0 {
        let monotonic = Duration::new(clock.tv_sec as u64, clock.tv_nsec as u32);
        now.checked_sub(monotonic.saturating_sub(timestamp)).unwrap_or(now)
    } else {
        now
    }
}

impl Outputs {
    pub(super) fn queue_panel(&mut self, back: usize, picture: Picture, now: Instant) {
        let flipped = self.read_panel_event();
        self.panel.stage(back, picture, now);
        self.service_panel(now, flipped);
    }

    pub(super) fn read_panel_event(&mut self) -> Option<Instant> {
        let events = self.disp.read_flips(0);
        self.vblank =
            events.vblank.map(|event| Flip { sequence: event.sequence, at: instant(event.monotonic) }).or(self.vblank);
        events.flip.map(|event| instant(event.monotonic))
    }

    pub(super) fn service_panel(&mut self, now: Instant, flipped: Option<Instant>) {
        self.panel.service(now, flipped, self.disp.refresh_period(), |back| self.disp.request_flip(back, now));
    }
}

#[cfg(test)]
mod tests;
