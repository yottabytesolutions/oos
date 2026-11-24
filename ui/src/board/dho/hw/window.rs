// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! Plan captured-ring windows for live reads, STOP replay and deep measurements.

use super::plan::{raw_newest, LIVE_READ};
use oos_hal::dho::{hz, seq::Settings};

/// The part of a deep record the screen shows, as a read-out window (`Settings::window`: points, first point from the
/// trigger point), when it fits `LIVE_READ` points: the screen and a quarter of a screen on each side (at least 4096
/// points), widened to the trigger point and 4096 points around it for the software trigger (a delay moves the screen
/// away from it).
/// The record is centred on the screen (`hz::prep`: it starts half a record before the delay). None with the LA,
/// which reads its own window of the same record.
pub(super) fn screen_window(s: &Settings, p: &hz::Params) -> Option<(u32, i64)> {
    window_below(s, p, LIVE_READ as i64)
}

/// `screen_window` for windows of fewer than `cap` points.
fn window_below(s: &Settings, p: &hz::Params, cap: i64) -> Option<(u32, i64)> {
    let screen = (s.tdiv * 10.0 * p.rate as f64).ceil() as i64;
    window_with(s, p, cap, (screen / 4).max(4096))
}

/// The screen and `margin` points each side (at least 4096 around the trigger), fewer than `cap` points.
fn window_with(s: &Settings, p: &hz::Params, cap: i64, margin: i64) -> Option<(u32, i64)> {
    if s.la.is_some() || p.rate == 0 {
        return None;
    }
    let screen = (s.tdiv * 10.0 * p.rate as f64).ceil() as i64;
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
    let mut r = oos_hal::dho::seq::raw_settings(b);
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

/// Whether arming ahead (`acq.prearm`) leaves the replay at STOP possible: not for a batch of the live compression or
/// one whose newest frame the deep measurements may read raw (`deep_raw_settings`), since the next batch would write
/// the ring under it. Prearm gains at
/// short time bases; deep compressed records take most of a batch in the read anyway.
#[cfg(any(target_os = "linux", test))]
pub(super) fn prearm_keeps_replay(b: &Settings, p: &hz::Params, deep: Option<crate::deepmeas::Want>) -> bool {
    !raw_newest(b, p) && (deep.is_none() || deep_raw_settings(b, p).is_none())
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
    if raw_newest(b, p) {
        Some(oos_hal::dho::seq::raw_settings(b))
    } else if deep {
        deep_raw_settings(b, p)
    } else {
        None
    }
}

/// The raw read of a deep record's screen window for the measurements (deepmeas.rs): the batch `b` read the whole
/// record through the compression (`batch_plan`, above 1 M points), Normal acquisition, no LA; the window of
/// `screen_window` with the record's edge allowed, at most `deepmeas::MAX_BYTES`. Settings for `seq::read_raw_again`.
pub(in crate::board::dho) fn deep_raw_settings(b: &Settings, p: &hz::Params) -> Option<Settings> {
    if b.compress < 2
        || b.window.is_some()
        || p.memsel <= crate::compress::MAX_DEPTH
        || !matches!(b.mode, oos_hal::dho::acq::Mode::Normal)
    {
        return None;
    }
    let cap = (crate::deepmeas::MAX_BYTES / (2 * p.ratio.max(1) as usize)) as i64 + 1;
    // the screen and 4096 points each side (the measurements need the screen, the trigger search the points around
    // the trigger): the quarter-screen margins of `screen_window` added half again to the DMA and the decode, which
    // blocked the acquisition about 186 ms a second at 10 M (unit 2026-11-14); the whole record where the window
    // would reach past it
    let w = window_with(b, p, cap, 4096)
        .or_else(|| ((p.memsel as i64) < cap && b.la.is_none()).then(|| (p.memsel, -(p.pre as i64))))?;
    Some(oos_hal::dho::seq::raw_settings(&Settings { window: Some(w), ..b.clone() }))
}
