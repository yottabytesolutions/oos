// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! Select the part of a captured record to play.

use super::Replay;
use oos_hal::dho::{
    acq::Mode,
    hz,
    seq::{self, Settings},
};

/// Target points per channel when compressing a deep record (stock 2.5 M / 4).
const LIVE_READ: u32 = 625_000;

/// Requested playback values. HAL resolves their effective window, factor and frame length.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct Play {
    window: Option<(u32, i64)>,
    compress: u32,
}

impl Play {
    pub(super) fn from_settings(s: &Settings) -> Self {
        Self { window: s.window, compress: s.compress }
    }

    /// Select the screen window for deep records when it fits. Otherwise request full-record compression.
    pub(super) fn select(s: &Settings, stored: &hz::Params) -> Self {
        if matches!(s.mode, Mode::Reduced(_)) {
            return Self { window: None, compress: 1 };
        }
        let mut play = Self::from_settings(s);
        if stored.memsel > crate::compress::MAX_DEPTH {
            match screen_window(s, stored) {
                Some(window) => play.window = Some(window),
                None => play.compress = stored.memsel.div_ceil(LIVE_READ),
            }
        }
        play
    }

    pub(super) fn apply(self, s: &mut Settings) {
        s.window = self.window;
        s.compress = self.compress;
    }

    pub(super) fn frame_bytes(self, s: &Settings, stored: &hz::Params) -> usize {
        if self == Self::from_settings(s) {
            return seq::frame_bytes(s, stored);
        }
        seq::frame_bytes(&self.playback(s), stored)
    }

    fn playback(self, s: &Settings) -> Settings {
        // The HAL byte count needs only the analog playback fields. Avoid cloning the mask and zone tables.
        Settings {
            playback: s.playback,
            header: s.header,
            window: self.window,
            compress: self.compress,
            ..Settings::default()
        }
    }

    pub(super) fn replay(self, mode: Mode, stored: &hz::Params) -> Replay {
        if self.compress < 2 {
            Replay::None
        } else if stored.memsel <= crate::compress::MAX_DEPTH {
            Replay::Raw
        } else if self.window.is_none() && mode == Mode::Normal {
            Replay::DeepWindow
        } else {
            Replay::None
        }
    }
}

/// Select an aligned live crop with the same compression factor and pair phase as the whole record.
pub(in crate::board::dho::hw) fn live_window(
    s: &Settings,
    p: &hz::Params,
    columns: usize,
) -> Result<Option<(u32, i64)>, seq::ReadoutError> {
    if s.compress < 2
        || p.memsel > crate::compress::MAX_DEPTH
        || !s.playback.is_play4()
        || s.mode != Mode::Normal
        || s.single
        || s.segments.is_some()
        || s.la.is_some()
        || s.window.is_some()
        || p.max_frames < 2
        || p.rate == 0
    {
        return Ok(None);
    }
    let screen = crate::model::HDIV as f64 * s.tdiv * p.rate as f64;
    let k = seq::compress_k(s, p);
    if k < 2 {
        return Ok(None);
    }
    let margin = f64::from(live_margin(screen, k, columns));
    let pre = f64::from(p.pre);
    let center = pre + s.delay * p.rate as f64;
    let left = (center - screen / 2.0).min(pre) - margin;
    let right = (center + screen / 2.0).max(pre) + margin;
    // An eight-output-point group preserves both the whole-record pair phase and its floor-rounded tail.
    let group = 8 * k;
    let lo = (left.floor() as u32).min(p.memsel) / group * group;
    let hi = (right.ceil() as u32).max(lo).min(p.memsel);
    let n = (hi - lo).div_ceil(group).saturating_mul(group).min(p.memsel - lo);
    if n == p.memsel {
        return Ok(None);
    }
    let window = Some((n, i64::from(lo) - i64::from(p.pre)));
    let playback = Play { window, compress: s.compress }.playback(s);
    if seq::compress_k(&playback, p) != k {
        return Err(seq::ReadoutError::InsufficientPoints { points: n, factor: k, len: 8 });
    }
    seq::validate_readout(&playback, p)?;
    Ok(window)
}

/// Cropped placement stays within the near search. Reserve its smoothed sample range and the
/// interpolation support so a placed screen can exceed the crop only at a whole-record boundary.
pub(super) fn live_margin(screen: f64, k: u32, columns: usize) -> u32 {
    let l = crate::compress::smooth_len(f64::from(k) * columns as f64 / screen);
    (crate::trignear::near_span(l) + l + oos_dsp::interp::LANCZOS8_RADIUS) as u32 * k
}

/// The part of a deep record the screen shows, as a read-out window (`Settings::window`: points, first point from the
/// trigger point), when it fits `LIVE_READ` points: the screen and a quarter of a screen on each side (at least 4096
/// points), widened to the trigger point and 4096 points around it for the software trigger (a delay moves the screen
/// away from it).
/// The record is centred on the screen (`hz::prep`: it starts half a record before the delay). None with the LA,
/// which reads its own window of the same record.
fn screen_window(s: &Settings, p: &hz::Params) -> Option<(u32, i64)> {
    let screen = screen_points(s, p);
    window_with(s, p, LIVE_READ as i64, screen, (screen / 4).max(4096))
}

fn screen_points(s: &Settings, p: &hz::Params) -> i64 {
    (s.tdiv * 10.0 * p.rate as f64).ceil() as i64
}

/// The screen and `margin` points each side (at least 4096 around the trigger), fewer than `cap` points.
fn window_with(s: &Settings, p: &hz::Params, cap: i64, screen: i64, margin: i64) -> Option<(u32, i64)> {
    if s.la.is_some() || p.rate == 0 {
        return None;
    }
    let (mem, pre, mid) = (p.memsel as i64, p.pre as i64, p.memsel as i64 / 2);
    // a multiple of 8 points like the stock TX length, inside the record
    let lo = ((mid - screen / 2 - margin).min(pre - 4096).max(0)) / 8 * 8;
    let hi = (mid + screen / 2 + margin).max(pre + 4096).min(mem);
    let n = (hi - lo + 7) / 8 * 8;
    if n >= cap || lo + n > mem {
        return None;
    }
    Some((n as u32, lo - pre))
}

/// STOP: what of the stored record (parameters `p`, read live with the settings `b`) to read again for a view of
/// `tdiv` per division at `delay`, and where the trigger lies in the frame that read gives (in its points, may be
/// outside it). The view with a quarter screen on each side at every sample when that fits `LIVE_READ` points; else
/// the whole record through the FPGA compression (as the live read of a long record). The record is laid out from
/// the trigger point `p.pre` at `p.rate`, so any part of it can be read, the trigger need not be in the window.
pub fn stop_plan(b: &Settings, p: &hz::Params, tdiv: f64, delay: f64) -> (Settings, f64) {
    let mut r = seq::raw_settings(b);
    r.window = None;
    let rate = p.rate as f64;
    let screen = (tdiv * 10.0 * rate).ceil();
    let margin = (screen / 4.0).max(4096.0);
    let (mem, pre) = (p.memsel as f64, p.pre as f64);
    let centre = pre + delay * rate;
    let lo = ((centre - screen / 2.0 - margin).max(0.0) / 8.0).floor() * 8.0;
    let hi = (centre + screen / 2.0 + margin).min(mem);
    if hi > lo {
        let n = ((hi - lo) / 8.0).ceil() * 8.0;
        if n < LIVE_READ as f64 && lo + n <= mem {
            r.window = Some((n as u32, lo as i64 - p.pre as i64));
            return (r, pre - lo);
        }
    }
    if p.memsel > LIVE_READ && r.playback.is_play4() {
        r.compress = p.memsel.div_ceil(LIVE_READ);
    }
    let k = r.compress.max(1) as f64;
    (r, pre / k)
}

/// Raw screen window for deep measurements after playing the whole record through compression.
pub(super) fn deep_replay_window(b: &Settings, p: &hz::Params) -> Option<(u32, i64)> {
    let cap = (crate::deepmeas::MAX_BYTES / (2 * p.ratio.max(1) as usize)) as i64 + 1;
    // the screen and 4096 points each side (the measurements need the screen, the trigger search the points around
    // the trigger): the quarter-screen margins of `screen_window` added half again to the DMA and the decode, which
    // blocked the acquisition about 186 ms a second at 10 M (unit 2026-11-14); the whole record where the window
    // would reach past it
    window_with(b, p, cap, screen_points(b, p), 4096)
        .or_else(|| ((p.memsel as i64) < cap && b.la.is_none()).then(|| (p.memsel, -(p.pre as i64))))
}
