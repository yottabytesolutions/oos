// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! Roll/scan mode selection and stream geometry.

use crate::{board::Slow, model::*};

/// Time base from which the hardware source streams in roll mode: 200 ms/div (stock: 50 ms/div). Below it the stream
/// comes out too fast for its time axis with two or four channels, a normal acquisition is right there.
pub const ROLL_TDIV: f64 = 0.2;
/// Time base from which the hardware source sweeps in scan mode when Auto Roll is off (stock: 200 ms/div).
pub const SCAN_TDIV: f64 = 0.2;

/// The OOS switching rule: Auto Roll on -> roll from 200 ms/div; off -> scan from 200 ms/div unless averaging.
pub fn slow_mode(m: &Model) -> Slow {
    // f32 time bases: 5 ms is 4.99999989 ms
    let tdiv = m.hw_time_per_div() as f64 * 1.0001;
    if m.auto_acquisition.roll() && tdiv >= ROLL_TDIV {
        Slow::Roll
    } else if !m.auto_acquisition.roll() && tdiv >= SCAN_TDIV && m.acq != AcqMode::Average {
        Slow::Scan { delay: m.delay as f64, single: m.trig_mode == TrigMode::Single }
    } else {
        Slow::Off
    }
}
/// Time of the first LA word from analog point `pre`. The newest word (index `n_la` - 1,
/// spacing `la_dt`) sits at the time of the newest analog point (index `len` - 1, spacing `dt`).
#[cfg(any(target_os = "linux", test))]
pub fn roll_la_t0(len: usize, pre: usize, dt: f64, n_la: usize, la_dt: f64) -> f64 {
    (len as f64 - 1.0 - pre as f64) * dt - (n_la as f64 - 1.0).max(0.0) * la_dt
}
/// Writes the scan points `v` into the sweep picture (`width` points) from `at`, wrapping at the right edge.
#[cfg(any(target_os = "linux", test))]
pub(super) fn scan_write(pic: &mut Vec<f32>, width: usize, at: usize, v: &[f32]) {
    if pic.len() != width {
        *pic = vec![f32::NAN; width];
    }
    for (i, &y) in v.iter().enumerate() {
        pic[(at + i) % width] = y;
    }
}

/// Where the LA words of a chunk go in the sweep picture: the same place on the screen as the analog point `cursor`.
#[cfg(any(target_os = "linux", test))]
pub(super) fn scan_la_pos(cursor: usize, keep: usize, keep_la: usize) -> usize {
    ((cursor as f64 * keep_la as f64 / keep.max(1) as f64).round() as usize) % keep_la.max(1)
}

/// Blanks `gap` points from the write position (the old sweep right of it stays visible).
#[cfg(any(target_os = "linux", test))]
pub(super) fn scan_gap(pic: &mut [Vec<f32>; NCH], at: usize, gap: usize) {
    for v in pic.iter_mut().filter(|v| !v.is_empty()) {
        let w = v.len();
        for i in 0..gap.min(w) {
            v[(at + i) % w] = f32::NAN;
        }
    }
}
