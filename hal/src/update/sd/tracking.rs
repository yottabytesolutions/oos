// SPDX-License-Identifier: GPL-2.0-or-later

use super::Mode;
use crate::bootcard::{Mode as Timing, Status};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(in crate::update) struct Tracking {
    pub(in crate::update) failed: bool,
    pub(in crate::update) missed: u8,
    /// The requested tree of the pending handoff or recovery trial, including same-build restages.
    pub(in crate::update) trial: Option<Mode>,
    /// A known timing from this attempt, including a warm slowdown that leaves SDR104 unverified.
    pub(in crate::update) observed: bool,
}

impl Tracking {
    pub(in crate::update) fn begin(&mut self, mode: Option<Mode>) {
        self.trial = mode;
        self.observed = false;
    }

    pub(in crate::update) fn observe(&mut self, status: Status, warm: bool, on_trial: bool) -> bool {
        let before = *self;
        match status.mode() {
            Timing::Sdr104 => self.missed = 0,
            Timing::Unknown => return false,
            _ if status.ceiling() != Timing::Sdr104 => return false,
            _ => self.failed |= !warm,
        }
        self.observed |= on_trial && self.trial == Some(Mode::Sdr104);
        *self != before
    }

    /// Consume the attempt and, when abandoned, count it only if it yielded no known timing.
    pub(in crate::update) fn finish(&mut self, abandoned: bool) {
        if abandoned && self.trial == Some(Mode::Sdr104) && !self.observed {
            self.missed = (self.missed + 1).min(2);
            self.failed |= self.missed == 2;
        }
        self.begin(None);
    }

    pub(super) fn retry(&mut self) -> bool {
        let changed = self.failed || self.missed != 0;
        self.failed = false;
        self.missed = 0;
        changed
    }
}
