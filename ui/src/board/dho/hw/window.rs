// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! Replay requests for the captured ring.

#![cfg(any(target_os = "linux", test))]

use super::read_plan::ReadPlan;
use oos_hal::dho::{hz, seq::Settings};

pub use super::read_plan::stop_plan;

/// Whether arming ahead (`acq.prearm`) leaves the replay at STOP possible: not for a batch of the live compression or
/// one whose newest frame the deep measurements may read raw (`deep_raw_settings`), since the next batch would write
/// the ring under it. Prearm gains at
/// short time bases; deep compressed records take most of a batch in the read anyway.
pub(super) fn prearm_keeps_replay(b: &Settings, p: &hz::Params, deep: Option<crate::deepmeas::Want>) -> bool {
    let plan = ReadPlan::from_batch(b, p);
    !plan.raw_newest() && (deep.is_none() || plan.deep_replay_window(b).is_none())
}

/// The raw read at STOP (docs/fork/stop-exact.md) of the newest frame of the batch `b` with `count` frames read just
/// before, when that frame was read as pairs and not already raw and nothing was armed ahead (`ahead`: the ring is
/// being written again): the whole frame for the live compression, the screen window for a deep record when the deep
/// measurements want samples.
pub(in crate::board::dho) fn final_replay(
    b: &Settings,
    p: &hz::Params,
    count: u32,
    newest_raw: bool,
    ahead: bool,
    deep: Option<crate::deepmeas::Want>,
) -> Option<Settings> {
    if count == 0 || newest_raw || ahead {
        return None;
    }
    raw_replay_settings(b, p, deep.is_some())
}

/// The live-compression replay or, when requested, the deep raw window.
pub(in crate::board::dho) fn raw_replay_settings(b: &Settings, p: &hz::Params, deep: bool) -> Option<Settings> {
    ReadPlan::from_batch(b, p).replay_settings(b, deep)
}

/// Read the deep raw window for measurements (deepmeas.rs).
pub(in crate::board::dho) fn deep_raw_settings(b: &Settings, p: &hz::Params) -> Option<Settings> {
    ReadPlan::from_batch(b, p).deep_replay_settings(b)
}
