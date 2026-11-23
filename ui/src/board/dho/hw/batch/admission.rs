// SPDX-License-Identifier: GPL-2.0-or-later
//! Combine the plan's hardware, plane and measured work bounds before arm.

use super::*;
use crate::drawpipe::admission::Grant;
use std::time::Duration;

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
    budget: Duration,
) -> Grant {
    let bound = cap.apply(uncapped);
    if pipe.is_some() {
        read_grant(pipe, identity, bound, budget)
    } else {
        crate::board::dho::publication::record_grant(out, identity, bound, settings, params, budget)
    }
}

#[cfg(test)]
mod tests;
