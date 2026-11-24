// SPDX-License-Identifier: GPL-2.0-or-later
//! Time budgets of the frame pipeline (docs/architecture/frames.md, admission). One picture period; the pool work one
//! batch may need; and read done to publication.

#[cfg(test)]
use crate::pacing::TIMER_PERIOD;
use std::time::Duration;

// Timer-based host fixtures use the legacy cadence without publishing a device clock.
#[cfg(test)]
pub(crate) const PICTURE_PERIOD: Duration = TIMER_PERIOD;
#[cfg(test)]
pub(crate) const WORK_BUDGET: Duration = default_work_budget(TIMER_PERIOD);

const fn percent(period: Duration, ratio: u128) -> Duration {
    Duration::from_nanos(((period.as_nanos() * ratio + 50) / 100) as u64)
}

/// Default service time one submitted batch may need.
pub(crate) const fn default_work_budget(period: Duration) -> Duration {
    percent(period, 36)
}

/// Read completion to publication, with room for one batch ahead of this one.
pub(crate) fn pipeline_budget(period: Duration) -> Duration {
    percent(period, 75)
}
