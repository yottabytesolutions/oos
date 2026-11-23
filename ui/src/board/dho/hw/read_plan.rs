// SPDX-License-Identifier: GPL-2.0-or-later

//! Read geometry for a configured acquisition, using the stored geometry returned by HAL.

#[cfg(any(target_os = "linux", test))]
use oos_hal::dho::{
    acq::Mode,
    hz,
    seq::{self, Settings},
};
#[cfg(any(target_os = "linux", test))]
use window::Play;

mod capture;
mod depth;
#[cfg(any(target_os = "linux", test))]
mod window;

#[cfg(test)]
mod tests;

pub(super) use capture::configure_capture;
pub(super) use depth::depth;
#[cfg(any(target_os = "linux", test))]
pub use window::stop_plan;

#[cfg(test)]
pub(super) use {
    capture::live_compress,
    depth::{screen_margin, LIVE_DEPTH},
};

#[cfg(any(target_os = "linux", test))]
pub(super) struct ReadPlan<'a> {
    stored: &'a hz::Params,
    play: Play,
    frames: u32,
    replay: Replay,
}

#[cfg(any(target_os = "linux", test))]
#[derive(Clone, Copy, PartialEq, Eq)]
enum Replay {
    None,
    Raw,
    DeepWindow,
}

#[cfg(any(target_os = "linux", test))]
impl<'a> ReadPlan<'a> {
    /// Recover the plan of an existing batch, including any changes made after initial planning.
    pub(super) fn from_batch(s: &Settings, stored: &'a hz::Params) -> Self {
        Self::with_play(s, stored, Play::from_settings(s))
    }

    fn with_play(s: &Settings, stored: &'a hz::Params, play: Play) -> Self {
        Self { stored, play, frames: s.frames, replay: play.replay(s.mode, stored) }
    }

    pub(super) fn apply(&self, s: &mut Settings) {
        self.play.apply(s);
        s.frames = self.frames;
    }

    pub(super) fn raw_newest(&self) -> bool {
        self.replay == Replay::Raw
    }

    pub(super) fn replay_settings(&self, b: &Settings, deep: bool) -> Option<Settings> {
        if self.raw_newest() {
            Some(seq::raw_settings(b))
        } else if deep {
            self.deep_replay_settings(b)
        } else {
            None
        }
    }

    pub(super) fn deep_replay_settings(&self, b: &Settings) -> Option<Settings> {
        let window = self.deep_replay_window(b)?;
        let mut replay = seq::raw_settings(b);
        replay.window = Some(window);
        Some(replay)
    }

    pub(super) fn deep_replay_window(&self, b: &Settings) -> Option<(u32, i64)> {
        if self.replay != Replay::DeepWindow {
            return None;
        }
        window::deep_replay_window(b, self.stored)
    }
}

/// Decide what to play and how many records fit in one batch of the configured capture.
#[cfg(any(target_os = "linux", test))]
pub(super) fn read_plan<'a>(s: &Settings, stored: &'a hz::Params, zone_batch: u32) -> ReadPlan<'a> {
    let play = Play::select(s, stored);
    let mut plan = ReadPlan::with_play(s, stored, play);
    plan.frames = batch_frames(s, stored, play.frame_bytes(s, stored).max(1), zone_batch);
    plan
}

#[cfg(any(target_os = "linux", test))]
fn batch_frames(s: &Settings, stored: &hz::Params, frame_bytes: usize, zone_batch: u32) -> u32 {
    use super::super::{BATCH_MAX, PEAK_BATCH_MAX};
    // Hardware average reads one frame regardless of the batch size.
    let by_size = if matches!(s.mode, Mode::Average { .. }) { u32::MAX } else { (4_000_000 / frame_bytes) as u32 };
    let mut frames = by_size.min(stored.max_frames).min(BATCH_MAX as u32 / stored.ratio.max(1)).max(1);
    if s.mode.is_peak() {
        frames = frames.min(PEAK_BATCH_MAX as u32);
    }
    // SINGLE takes the first trigger after the press. Waiting for a full batch added up to 25 ms at 1 us/div
    // (unit, 2026-11-13).
    if s.single {
        frames = 1;
    }
    // The zone trigger captures one qualified frame per arm (docs/fork/hwzone.md, "Rate").
    if s.hwzone.is_some() {
        frames = frames.min(zone_batch);
    }
    frames
}
