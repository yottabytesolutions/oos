// SPDX-License-Identifier: GPL-2.0-or-later
//! Combine the plan's hardware, plane and measured work bounds before arm.

use super::*;
use crate::drawpipe::admission::{Grant, GrantBudget, WindowCycle};
use std::time::Duration;

#[cfg(test)]
pub(in crate::board::dho::hw) fn read_grant(
    pipe: Option<&Pipe>,
    identity: Identity,
    bound: u32,
    budget: Duration,
) -> Grant {
    pipe.map_or(Grant::Overlapping(bound), |pipe| pipe.grant(identity, bound, budget))
}

#[cfg(any(target_os = "linux", test))]
pub(in crate::board::dho::hw) fn grant(
    (pipe, uncapped): (Option<&Pipe>, u32),
    cap: &crate::pacing::FrameCap,
    (out, identity): (&crate::board::HwShared, Identity),
    (settings, params): (&Settings, &hz::Params),
    (work, period): (Duration, impl FnOnce() -> Duration),
) -> Grant {
    let cycle =
        super::super::window::is_live_crop(settings, params).then(|| window_cycle(cap, settings, params, period()));
    let budget = GrantBudget::with_window(work, cycle);
    let bound = if cycle.is_some() { uncapped } else { cap.apply(uncapped) };
    if let Some(pipe) = pipe {
        pipe.grant(identity, bound, budget)
    } else {
        crate::board::dho::publication::record_grant(out, identity, bound, settings, params, budget)
    }
}

fn window_cycle(cap: &crate::pacing::FrameCap, s: &Settings, p: &hz::Params, period: Duration) -> WindowCycle {
    use oos_hal::dho::seq;
    let (points, _) = seq::window(s, p);
    let (fixed, dma) = cap.window_read(u64::from(points) * u64::from(p.ratio));
    let capture = Duration::from_nanos((u64::from(p.memsel) * 1_000_000_000).div_ceil(p.rate.max(1)));
    let holdoff = Duration::try_from_secs_f64(s.holdoff).unwrap_or(Duration::ZERO);
    let per_record = capture.saturating_add(holdoff).saturating_add(dma);
    WindowCycle::new(
        crate::drawpipe::budget::pipeline_budget(period),
        fixed,
        per_record,
        seq::compressed_len(s, p).into(),
    )
}

#[cfg(test)]
mod tests;
