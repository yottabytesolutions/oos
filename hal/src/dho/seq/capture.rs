// SPDX-License-Identifier: GPL-2.0-or-later

//! X2 shares the ordinary capture and play sequences. Only their horizontal parameters and mode change.

use super::{tdiv_fs, time_fs, Settings, State, Step, Trig};
use crate::dho::{acq::Mode, hz};
use crate::dho::{clock, spu};
use std::{fmt, time::Duration};

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
        if !ordinary_capture(s, self.peak) || clock::overclocked() {
            return None;
        }
        let max_rate = hz::CLOCK as u64 / u64::from(full.ratio);
        let rate = max_rate / u64::from(self.factor);
        // The slow-path divider is relative to ADC/20 and cannot make these power-of-two rates exactly.
        if rate >= full.rate
            || (!self.peak && full.rate != max_rate)
            || spu::slow_path(rate)
            || rate * u64::from(self.factor) != max_rate
        {
            return None;
        }
        if !self.inferred {
            return if self.peak { None } else { stock_params(s.mask, full, self.factor, rate) };
        }
        // Preserve the unreduced time span to within one TX group, even after an Auto rate remap.
        let points = (u64::from(full.memsel) * rate / full.rate / 8 * 8) as u32;
        let p = hz::centered_params(full.ratio, rate, points);
        (points >= 8 && capture_window(s, &p)).then_some(p)
    }
}

impl fmt::Display for CaptureReduction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", if self.peak { "peak" } else { "decimate" }, self.factor)
    }
}

fn ordinary_capture(s: &Settings, peak: bool) -> bool {
    (matches!(s.mask, 1 | 3 | 15) || (peak && s.mask == 2))
        && s.delay == 0.0
        && matches!(s.trigger, Trig::Edge)
        && s.la.is_none()
        && s.segments.is_none()
        && s.hwmask.is_none()
        && s.hwzone.is_none()
        && s.blocks == 7
        && s.playback.is_play4()
        && (s.window.is_none() || peak)
        && s.compress == 1
}

fn capture_window(s: &Settings, p: &hz::Params) -> bool {
    let Some((points, offset)) = s.window else { return true };
    let trigger = i64::from(p.pre);
    let start = trigger.saturating_add(offset);
    let end = start.saturating_add(i64::from(points));
    points >= 8
        && points.is_multiple_of(8)
        && start % 8 == 0
        && start >= 0
        && start <= trigger
        && trigger < end
        && end <= i64::from(p.memsel)
}

impl CaptureReduction {
    /// Arm two slots after changing geometry. Slot zero may still hold the previous arm's samples.
    pub fn arm_transition(state: &mut State, s: &Settings, p: &hz::Params) -> Vec<Step> {
        state.arm_only_steps(&Settings { frames: 2, ..s.clone() }, p)
    }

    /// Play just slot one once complete. A lack of triggers returns `Ok(None)`.
    pub fn read_transition(
        state: &mut State,
        s: &Settings,
        p: &hz::Params,
        bar: &mut impl crate::dho::bus::Bus,
        clock: &impl super::time::Clock,
        stop: impl Fn() -> bool,
    ) -> Result<Option<Vec<Step>>, String> {
        // The source admits transitions only with steady triggers. Allow several trigger
        // periods plus the time to store both records.
        let timeout = Duration::from_millis(100) + super::wait::record_time(p.memsel, p.rate) * 2;
        let deadline = clock.now() + timeout;
        loop {
            if stop() {
                return Err(super::NEW_SETTINGS.into());
            }
            let info = bar.try_rd(0x4080).map_err(|error| error.to_string())?;
            if info != u32::MAX && info & crate::dho::scu::PLAYINFO_DONE != 0 {
                let position = bar.try_rd(super::wait::RECORD_POSITION).map_err(|error| error.to_string())?;
                match position {
                    1 => return state.read_steps_at(s, p, 1, 1).map(Some).map_err(|error| error.to_string()),
                    0 | u32::MAX => {}
                    _ => return Err(format!("capture transition: expected ring slot 1, got {position:#x}")),
                }
            }
            let remaining = deadline.saturating_duration_since(clock.now());
            if remaining.is_zero() {
                return Ok(None);
            }
            clock.wait(remaining.min(Duration::from_micros(500)));
        }
    }
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
    /// Select the largest inferred peak divider 8, 4 or 2 that supplies the requested word rate.
    pub fn reduce_capture_auto(&mut self, min_word_rate: u64) -> Option<hz::Params> {
        let max_rate = hz::CLOCK as u64 / u64::from(hz::ratio(self.mask));
        [8, 4, 2]
            .into_iter()
            .filter(|factor| max_rate / u64::from(*factor) >= min_word_rate)
            .find_map(|factor| self.reduce_capture(true, factor, true))
    }

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

#[cfg(test)]
mod reduced_profile_tests {
    use super::*;

    #[test]
    fn auto_uses_exact_rates_and_retains_the_original_record_span() {
        for (mask, depth, tdiv, rate, divisor, points) in [
            (1, 14_064, 1e-6, 625_000_000, 2, 7032),
            (2, 14_064, 1e-6, 625_000_000, 2, 7032),
            (1, 100_000, 10e-6, 156_250_000, 8, 25_000),
            (3, 70_312, 10e-6, 78_125_000, 8, 8784),
            (15, 35_160, 10e-6, 78_125_000, 4, 8784),
        ] {
            let mut s = Settings {
                mask,
                depth,
                tdiv,
                trig_src: mask.trailing_zeros() + 1,
                window: None,
                ..Settings::default()
            };
            let full = horizontal(&s).unwrap();
            let min_word_rate = (400.0_f64 / tdiv).ceil() as u64;
            let p = s.reduce_capture_auto(min_word_rate).unwrap();
            assert!(matches!(s.mode, Mode::Reduced(capture) if capture.peak && capture.factor == divisor));
            assert_eq!((p.rate, p.memsel, p.pre, p.post), (rate, points, points / 2, points / 2));
            assert!(u64::from(full.memsel) * p.rate - u64::from(p.memsel) * full.rate < 8 * full.rate);
            let (steps, configured) = State::handover(mask, None).configure(&s, None, None).unwrap();
            assert_eq!(configured, p);
            assert!(steps.iter().any(|step| matches!(step, Step::Wr((0x10d0, value)) if *value == 0x8000 | divisor)));
        }
        for (mask, depth, tdiv) in [(1, 14_064, 100e-9), (3, 7032, 1e-6), (15, 3520, 1e-6), (1, 100_000, 100e-6)] {
            let mut s = Settings { mask, depth, tdiv, window: None, ..Settings::default() };
            assert!(s.reduce_capture_auto((400.0_f64 / tdiv).ceil() as u64).is_none());
            assert_eq!(s.mode, Mode::Normal);
        }
    }

    #[test]
    fn peak_can_play_an_aligned_window_at_pass_through_only() {
        let mut s = Settings { tdiv: 10e-6, depth: 100_000, window: None, ..Settings::default() };
        let p = s.reduce_capture(true, 8, true).unwrap();
        s.window = Some((16_000, 4096 - i64::from(p.pre)));
        let mut state = State::handover(s.mask, None);
        assert_eq!(state.configure(&s, None, None).unwrap().1, p);
        assert_eq!(super::super::window(&s, &p), (16_000, 4096));
        let read = state.read_steps(&s, &p, 3).unwrap();
        assert!(matches!(read.last(), Some(Step::Read { frames: 3, frame: 32_000 })));
        assert!(read.iter().any(|step| matches!(step, Step::Wr((0x1170, 0x4000_0001)))));
        s.compress = 2;
        assert!(state.configure(&s, None, None).is_err());
        s.compress = 1;
        for invalid in [(16_000, 4097 - i64::from(p.pre)), (16_004, -(p.pre as i64)), (p.memsel, 1)] {
            s.window = Some(invalid);
            assert!(state.configure(&s, None, None).is_err());
        }
    }
}
