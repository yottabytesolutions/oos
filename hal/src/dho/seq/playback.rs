// SPDX-License-Identifier: GPL-2.0-or-later

//! Inferred main-trace linear play for the stock display geometry.

use super::{acq::Mode, hz, tdiv_fs, Settings, Trig};

const COLUMNS: u32 = 1000;

/// The requested analog playback path.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Playback {
    /// Type-4 play with the ordinary OOS read parameters.
    #[default]
    Raw,
    /// Diagnostic play-last from a stock hand-over.
    Last,
    /// Type-4 display play restricted to observed profiles.
    Stock,
    /// Type-4 display play with explicit permission to use inferred geometry.
    StockInferred,
}

impl Playback {
    /// Whether reads use the type-4 play FSM.
    pub fn is_play4(self) -> bool {
        self != Self::Last
    }
}

/// Derived main window and TX geometry for the experimental linear play.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StockPlay {
    points: u32,
    start: u32,
    len: u32,
    step: u32,
}

impl StockPlay {
    /// Input points per channel and first input point in the stored record.
    pub fn window(self) -> (u32, u32) {
        (self.points, self.start)
    }

    /// Programmed TX points per channel (`0x11f0`).
    pub fn frame_len(self) -> u32 {
        self.len
    }

    /// Main linear interpolation step (`0x115c`).
    pub fn linear_step(self) -> u32 {
        self.step
    }

    pub(super) fn writes(self) -> [crate::dho::Write; 3] {
        // The pass-through writes initialized the phase and disabled zoom, sinc and x2 interpolation.
        [(0x1154, 2), (0x115c, self.step), (0x11fc, COLUMNS)]
    }
}

/// Effective stock play geometry, if the capture and read use the supported profile.
pub fn stock_play(s: &Settings, p: &hz::Params) -> Option<StockPlay> {
    if s.playback != Playback::StockInferred || !eligible(s, p) {
        return None;
    }

    // Horizontal-config 3.1 documents linear resampling for fractional samples per column >= 1.
    // The sub-one-sample branch's sinc factor and phase are missing from the reconstruction.
    let tdiv = u128::try_from(tdiv_fs(s.tdiv)).ok()?;
    if !(1..=1_000_000_000).contains(&tdiv) {
        return None;
    }
    let numerator = tdiv * u128::from(p.rate);
    let denominator = 100 * hz::FS as u128;
    if numerator <= denominator || numerator % denominator == 0 {
        return None;
    }
    let points = u32::try_from((10 * numerator).div_ceil(hz::FS as u128)).ok()?;
    let per_column = u32::try_from(numerator.div_ceil(denominator)).ok()?;
    let len = per_column * COLUMNS;
    let step = u32::try_from(u128::from(u32::MAX) * numerator / (u128::from(per_column) * denominator)).ok()?;
    let start = p.pre.checked_sub(points.div_ceil(2))?;
    // Allow the linear interpolator's right neighbour even when the last phase lies past `points - 1`.
    let last = (u64::from(len - 1) * u64::from(step)).div_ceil(1 << 32);
    (u64::from(start) + last.max(u64::from(points - 1)) < u64::from(p.memsel)).then_some(StockPlay {
        points,
        start,
        len,
        step,
    })
}

fn eligible(s: &Settings, p: &hz::Params) -> bool {
    matches!(s.mask.count_ones(), 1 | 2 | 4)
        && s.mask <= 15
        && p.ratio == hz::ratio(s.mask)
        && i128::from(p.rate) * i128::from(p.ratio) == hz::CLOCK
        && !crate::dho::clock::overclocked()
        && s.mode == Mode::Normal
        && matches!(s.trigger, Trig::Edge)
        && s.delay == 0.0
        && s.ch.iter().all(|ch| ch.skew == 0.0)
        && s.blocks == 7
        && s.window.is_none()
        && s.compress == 1
        && s.la.is_none()
        && s.segments.is_none()
        && s.hwmask.is_none()
        && s.hwzone.is_none()
}
