// SPDX-License-Identifier: GPL-2.0-or-later

use std::time::Duration;

#[derive(Clone, Copy)]
pub(crate) struct GrantBudget {
    pub(super) work: Duration,
    pub(super) cycle: Option<WindowCycle>,
}

impl From<Duration> for GrantBudget {
    fn from(work: Duration) -> Self {
        Self { work, cycle: None }
    }
}

impl GrantBudget {
    pub(crate) fn with_window(work: Duration, cycle: Option<WindowCycle>) -> Self {
        Self { work, cycle }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct WindowCycle {
    budget: Duration,
    fixed: Duration,
    per_record: Duration,
    pub(super) played_points: u64,
}

impl WindowCycle {
    pub(crate) fn new(budget: Duration, fixed: Duration, per_record: Duration, played_points: u64) -> Self {
        Self { budget, fixed, per_record, played_points }
    }

    pub(super) fn limit(self, work: u128, fixed: u128, workers: u128) -> u128 {
        let available = self.budget.as_nanos().saturating_sub(fixed + self.fixed.as_nanos());
        let per_record = self.per_record.as_nanos().max(1);
        // N records must fit capture (full pre/post fill), re-arm, cropped read and placement in one pipeline budget.
        // Up to one record per worker, placement costs one record. After that it scales at the worker pool rate.
        let first_wave = available.saturating_sub(work) / per_record;
        if first_wave < workers {
            first_wave
        } else {
            available * workers / (per_record * workers + work)
        }
    }
}
