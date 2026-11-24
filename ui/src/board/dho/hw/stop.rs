// SPDX-License-Identifier: GPL-2.0-or-later

use super::{comparator::is_stall, dma_source::ArmPlan, Settings};
use crate::board::{Shared, Slow};
use oos_hal::dho::seq;
use std::{
    cell::Cell,
    time::{Duration, Instant},
};

pub(super) type Asked<'a> = (Option<&'a Settings>, Slow);

#[derive(Default)]
pub(super) struct FirstWait(Cell<Option<Instant>>);

impl FirstWait {
    pub(super) fn canceled(
        &self,
        g: &Shared,
        plan: &ArmPlan<'_>,
        asked: Asked<'_>,
        (auto, reserved): (bool, bool),
        now: Instant,
    ) -> bool {
        if plan.capture_changed(g, asked) || g.force || g.stop_view.is_some() || g.record_flow.shutdown {
            return true;
        }
        if !g.hold {
            self.0.set(None);
            return false;
        }
        if !plan.serial() {
            return reserved || !auto;
        }
        // Leave time for disarm, readout and publication before the UI's three-second deadline.
        let stopped = self.0.get().unwrap_or(now);
        self.0.set(Some(stopped));
        now.saturating_duration_since(stopped) >= Duration::from_secs(2)
    }
}

pub(super) fn admission_interrupted(g: &Shared, plan: &ArmPlan<'_>, asked: Asked<'_>) -> bool {
    plan.handoff_obsolete(g, asked) || g.force || g.cal_req.is_some()
}

pub(super) fn read_obsolete(g: &Shared, plan: &ArmPlan<'_>, asked: Asked<'_>) -> bool {
    plan.capture_changed(g, asked)
        || g.record_flow.shutdown
        || g.stop_view.is_some()
        || (plan.uses_record_queue()
            && !g.hold
            && (g.force || g.cal_req.is_some())
            && (!g.queue.is_empty() || g.record_flow.segment_bound() == Some(0)))
}

pub(super) fn needs_disarm(error: &str) -> bool {
    error == seq::NEW_SETTINGS || is_stall(error)
}

pub(super) fn held_request(g: &Shared, plan: &ArmPlan<'_>, asked: Asked<'_>, count: u32, ahead: bool) -> Option<u64> {
    (plan.serial() && (count == 1 || !plan.uses_record_queue()) && !ahead && g.hold && !read_obsolete(g, plan, asked))
        .then_some(g.final_req)
}

#[cfg(test)]
mod tests;
