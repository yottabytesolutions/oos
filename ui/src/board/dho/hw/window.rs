// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! Replay requests for the captured ring.

#![cfg(any(target_os = "linux", test))]

use super::read_plan::ReadPlan;
use oos_hal::dho::{
    hz,
    seq::{self, Settings},
};

pub use super::read_plan::stop_plan;

/// Choose the live crop once, before batch admission. A rejected crop leaves the original play intact.
pub(super) fn configured_play_window(s: &Settings, p: &hz::Params, columns: usize) -> Option<(u32, i64)> {
    match super::read_plan::live_window(s, p, columns) {
        Ok(window) => window,
        Err(error) => {
            crate::drawpipe::counters().acquisition.reject_play_window();
            eprintln!("oos-ui hw: played window rejected, using whole-record play: {error}");
            None
        }
    }
}

pub(super) fn batch_play(s: &Settings, p: &hz::Params, zone_batch: u32, window: Option<(u32, i64)>) -> Settings {
    let mut b = s.clone();
    if window.is_some() {
        b.window = window;
    }
    super::read_plan::read_plan(&b, p, zone_batch).apply(&mut b);
    b
}

pub(super) fn is_live_crop(s: &Settings, p: &hz::Params) -> bool {
    s.window.is_some() && p.memsel <= crate::compress::MAX_DEPTH && seq::compress_k(s, p) > 1
}

/// Newest-record replay for readers and STOP, independent of the live display crop.
pub(super) fn whole_raw_replay(b: &Settings) -> Settings {
    let mut raw = seq::raw_settings(b);
    raw.window = None;
    raw
}

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
