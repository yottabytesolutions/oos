// SPDX-License-Identifier: GPL-2.0-or-later

//! X2 shares the ordinary capture and play sequences. Only their horizontal parameters and mode change.

use super::{tdiv_fs, time_fs, Settings, Trig};
use crate::dho::{acq::Mode, hz};
use crate::dho::{clock, spu};
use std::fmt;

/// An accepted X2 reduction request. Construct it with `Settings::reduce_capture`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaptureReduction {
    peak: bool,
    factor: u32,
    inferred: bool,
}

impl CaptureReduction {
    /// Whether the plan uses the explicitly opted-in geometry and Peak assumptions.
    pub fn inferred(self) -> bool {
        self.inferred
    }

    pub(in crate::dho) fn is_peak(self) -> bool {
        self.peak
    }

    fn params(self, s: &Settings, full: &hz::Params) -> Option<hz::Params> {
        if !ordinary_capture(s) || clock::overclocked() {
            return None;
        }
        let max_rate = hz::CLOCK as u64 / u64::from(full.ratio);
        let rate = max_rate / u64::from(self.factor);
        // The slow-path divider is relative to ADC/20 and cannot make these power-of-two rates exactly.
        if full.rate != max_rate || spu::slow_path(rate) || rate * u64::from(self.factor) != max_rate {
            return None;
        }
        if !self.inferred {
            return if self.peak { None } else { stock_params(s.mask, full, self.factor, rate) };
        }
        // Align down to whole TX groups. Peak assumes one 16-bit word per programmed point and lane.
        let points = full.memsel / self.factor / 8 * 8;
        (points >= 8).then(|| hz::centered_params(full.ratio, rate, points))
    }
}

impl fmt::Display for CaptureReduction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", if self.peak { "peak" } else { "decimate" }, self.factor)
    }
}

fn ordinary_capture(s: &Settings) -> bool {
    matches!(s.mask, 1 | 3 | 15)
        && s.delay == 0.0
        && matches!(s.trigger, Trig::Edge)
        && s.la.is_none()
        && s.segments.is_none()
        && s.hwmask.is_none()
        && s.hwzone.is_none()
        && s.blocks == 7
        && s.playback.is_play4()
        && s.window.is_none()
        && s.compress == 1
}

/// Exact tuples from the documented stock fixed-depth/remap rules (horizontal-config.md 1.1, 1.2 and 1.7).
fn stock_params(mask: u32, full: &hz::Params, factor: u32, rate: u64) -> Option<hz::Params> {
    let base_fs = match (full.ratio, factor) {
        (1, 2) => 100_000_000,
        (1, 4) | (2, 2) => 200_000_000,
        (1, 8) | (2, 4) | (4, 2) => 500_000_000,
        _ => return None,
    };
    // At the stock time base for a 1000-point record, scale time by depth to preserve the exact remapped rate.
    let target = full.memsel.div_ceil(factor);
    [1000, 10_000, 100_000]
        .into_iter()
        .filter(|points| (target..full.memsel).contains(points))
        .map(|points| hz::prep(mask, base_fs * i128::from(points) / 1000, i128::from(points), 0))
        .find(|p| p.rate == rate)
}

impl Settings {
    /// Select an X2 capture and return its stored geometry, leaving the settings unchanged if it cannot be planned.
    pub fn reduce_capture(&mut self, peak: bool, factor: u32, inferred: bool) -> Option<hz::Params> {
        let tdiv = tdiv_fs(self.tdiv);
        if tdiv == 0 || !matches!(factor, 2 | 4 | 8 | 16 | 32 | 64) || self.mode != Mode::Normal {
            return None;
        }
        let capture = CaptureReduction { peak, factor, inferred };
        let full = hz::prep(self.mask, tdiv, self.depth, 0);
        let params = capture.params(self, &full)?;
        self.mode = Mode::Reduced(capture);
        Some(params)
    }
}

pub(super) fn horizontal(s: &Settings) -> Result<hz::Params, String> {
    let tdiv = tdiv_fs(s.tdiv);
    let p = hz::prep(s.mask, tdiv, s.depth, 0);
    if let Mode::Reduced(capture) = s.mode {
        return capture.params(s, &p).ok_or_else(|| format!("incompatible settings for capture reduction {capture}"));
    }
    // With the LA on the record stays centred. Its rate rule follows this one.
    if s.delay != 0.0 && s.la.is_none() {
        let half = p.dot_fs as i128 * p.memsel as i128 / 2;
        return Ok(hz::prep(s.mask, tdiv, s.depth, time_fs(s.delay).clamp(-half, half)));
    }
    Ok(p)
}

#[cfg(test)]
mod tests;
