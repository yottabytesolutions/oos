// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! Select the requested memory depth before HAL configuration.

use crate::board::Slow;
use oos_hal::dho::{hz, seq::tdiv_fs};

/// Live record length for Auto depth (keeps the read short enough for a fluid display).
pub(in crate::board::dho::hw) const LIVE_DEPTH: i128 = 100_000;

/// Auto depth at short time bases: the stock record is exactly one screen at the full rate, so after the software
/// trigger moves the record by a few samples the edge of the screen has no data (acquisition-start.md, C2H read rules
/// 3). An eighth more memory (at least 64 points, a multiple of 8) at the same rate: `hz::prep` keeps the full rate for
/// a depth above one screen and centres the record on the trigger.
pub(in crate::board::dho::hw) fn screen_margin(memsel: i128) -> i128 {
    (memsel + (memsel / 8).max(64) + 7) / 8 * 8
}

/// Auto depth with a trigger delay: the record is centred on the delayed screen, but the trigger has to stay inside it
/// (`seq::State::configure` clamps the delay to half a record, trigger.md 4). A delay beyond half of the `base` record
/// then showed only the part of the screen up to the record's end (10 us/div, 200 us delay: 20 us of 100, unit
/// 2026-11-13). The record then gets the depth that reaches from the trigger (a division inside) past the delay at the
/// full rate; the depth limit of the channels still applies afterwards.
fn delay_depth(mask: u32, tdiv: f64, timebase_fs: i128, delay: f64, base: i128) -> i128 {
    let p = hz::prep(mask, timebase_fs, base, 0);
    let span = p.memsel as f64 / p.rate.max(1) as f64;
    if !delay.is_finite() || 2.0 * delay.abs() <= span {
        return base;
    }
    let full = (hz::CLOCK / hz::ratio(mask) as i128) as f64;
    // at most the deepest record of the board; `settings` then limits it to the channels' memory
    let want = ((2.0 * delay.abs() + 2.0 * tdiv) * full).ceil().min(125e6) as i128;
    ((want + 7) / 8 * 8).max(base)
}

pub(in super::super) fn depth(requested: i128, limit: i128, mask: u32, tdiv: f64, delay: f64, slow: Slow) -> i128 {
    let timebase_fs = tdiv_fs(tdiv);
    let depth = match requested {
        // roll streams without reading the memory: the stock Auto depth gives the stock point spacing (10 us at
        // 100 ms/div)
        0 if slow != Slow::Off => 0,
        0 => {
            let auto = hz::prep(mask, timebase_fs, 0, 0);
            let base = if auto.memsel as i128 > LIVE_DEPTH { LIVE_DEPTH } else { screen_margin(auto.memsel as i128) };
            delay_depth(mask, tdiv, timebase_fs, delay, base)
        }
        // roll and scan: a fixed depth below 1 M becomes 1 M (roll-mode.md 1.3)
        d if slow != Slow::Off => d.max(1_000_000),
        d => d,
    };
    // the menu depth may be from a setup with fewer channels
    let depth = depth.min(limit);
    // roll faster than 50 ms/div: no more than the roll stream's rate
    if slow != Slow::Roll {
        return depth;
    }
    let roll_limit = oos_hal::dho::roll::depth_for(timebase_fs);
    if roll_limit <= 0 {
        return depth;
    }
    if depth == 0 {
        roll_limit
    } else {
        depth.min(roll_limit)
    }
}
