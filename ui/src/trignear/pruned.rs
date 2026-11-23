// SPDX-License-Identifier: GPL-2.0-or-later

use super::{smooth_run, sum_window, Codes, EdgeSearch, Samples};
use crate::board::correct_adc_code;
use std::ops::Range;

const BLOCK: usize = 128;
const SUB_BLOCK: usize = 32;
pub(super) const MIN_SAMPLES: usize = BLOCK + SUB_BLOCK;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Band {
    BelowArm,
    BelowLevel,
    AboveLevel,
    AboveArm,
    Mixed,
}

impl Band {
    pub(super) fn of(lo: f32, hi: f32, trigger: EdgeSearch) -> Self {
        let (arm_r, arm_f) = (trigger.level - trigger.hysteresis, trigger.level + trigger.hysteresis);
        if hi < arm_r {
            Self::BelowArm
        } else if lo >= arm_r && hi < trigger.level {
            Self::BelowLevel
        } else if lo > arm_f {
            Self::AboveArm
        } else if lo > trigger.level && hi <= arm_f {
            Self::AboveLevel
        } else {
            Self::Mixed
        }
    }
}

/// Indices are relative to the original search origin, even across skipped samples.
pub(super) fn smooth(
    codes: &Codes<'_>,
    base: usize,
    bounds: Range<usize>,
    l: usize,
    trigger: EdgeSearch,
    (indexed, window): (&mut Vec<(usize, f32)>, &mut Vec<f64>),
) {
    if !(trigger.level.is_finite() && trigger.hysteresis >= 0.0 && trigger.hysteresis.is_finite()) {
        return;
    }
    let Some(offsets) = offset_bounds(codes) else { return };
    if !exact_sum(codes, l) {
        return;
    }
    let a = bounds.start;
    let end = bounds.end - l + 1;
    // Equal bands coalesce. Each bound covers every raw sample used by every window in the block.
    let mut current = None;
    let mut start = a;
    let mut emit = |range, band| emit_run(codes, base, a, l, (range, band), (indexed, window));
    let raw = Codes { corrected: None, ..*codes };
    let band_of = |first, last| {
        let (lo, hi) = raw.code_extremes(first - base, last + l - 1 - base);
        let (lo, hi) = codes.calibrated(correct_adc_code(lo, offsets.1), correct_adc_code(hi, offsets.0));
        Band::of(lo, hi, trigger)
    };
    let mut add = |first, band| {
        if let Some(previous) = current.filter(|&previous| previous != band) {
            emit(start..first, previous);
            start = first;
        }
        current = Some(band);
    };
    for first in (a..end).step_by(BLOCK) {
        let last = (first + BLOCK).min(end);
        let band = band_of(first, last);
        if band == Band::Mixed && l <= SUB_BLOCK / 4 {
            // For narrow averages, only refine the few blocks around the thresholds.
            for first in (first..last).step_by(SUB_BLOCK) {
                add(first, band_of(first, (first + SUB_BLOCK).min(last)));
            }
        } else {
            add(first, band);
        }
    }
    if let Some(band) = current.filter(|&band| start != a || band != Band::Mixed) {
        emit(start..end, band);
    }
}

fn emit_run(
    codes: &Codes<'_>,
    base: usize,
    origin: usize,
    l: usize,
    (range, band): (Range<usize>, Band),
    (indexed, window): (&mut Vec<(usize, f32)>, &mut Vec<f64>),
) {
    let x = |i| f64::from(codes.at(i - base));
    let mut keep = |i, average| indexed.push((i - origin, average));
    if band == Band::Mixed {
        smooth_run(range, l, window, x, |w| sum_window(w, 0.0), keep);
        return;
    }
    // The mean rounds between the raw extrema. Within one band both comparisons
    // have the same outcomes at every sample. Only the first can fire an edge,
    // and only the last can be the predecessor of an edge after the run. Keeping
    // both also preserves the first and last out-of-band indices of the near proof.
    let tail = range.end - 1;
    for i in std::iter::once(range.start).chain((tail != range.start).then_some(tail)) {
        let sum = (i..i + l).map(x).sum::<f64>();
        keep(i + l / 2, (sum / l as f64) as f32);
    }
}

fn offset_bounds(codes: &Codes<'_>) -> Option<(i32, i32)> {
    let Some(cal) = codes.corrected else { return Some((0, 0)) };
    let (&first, rest) = cal.offsets.split_first()?;
    let (lo, hi) = rest.iter().fold((first, first), |(lo, hi), &x| (lo.min(x), hi.max(x)));
    // Match the i32 subtraction in at_code throughout the word domain.
    i32::from(u16::MAX).checked_sub(lo)?;
    Some((lo, hi))
}

fn exact_sum(codes: &Codes<'_>, l: usize) -> bool {
    let volts = |word| codes.calibrated(word, word).0;
    let (a, b) = (volts(0), volts(u16::MAX));
    if !a.is_finite() || !b.is_finite() {
        return false;
    }
    let max = f64::from(a.abs().max(b.abs()));
    // If the largest intermediate is bounded by 2^E, and every nonzero f32
    // magnitude is >= 2^(E-29), all samples lie on a 2^(E-52) lattice. Thus
    // f64 represents every partial sum, difference, and updated sum exactly.
    // l + 2 bounds all three. The extra bit rounds the bound up to a power of
    // two conservatively. This lets us restart without inheriting any rounding
    // from a skipped prefix. Zero's sign cannot affect a crossing: the armed
    // comparator would already have fired if the predecessor equalled the level.
    let cutoff = max * (l as f64 + 2.0) * (1.0 / (1u64 << 28) as f64);
    let ordered = |i: u16| volts(if a <= b { i } else { u16::MAX - i });
    let (lo, hi) = (a.min(b), a.max(b));
    if lo > 0.0 || hi < 0.0 {
        return f64::from(lo.abs().min(hi.abs())) >= cutoff;
    }
    if lo == hi {
        return true;
    }
    let mid = first_nonnegative(ordered);
    let below = mid.checked_sub(1).is_none_or(|i| -f64::from(ordered(i)) >= cutoff);
    let above = ordered(mid);
    let above = if above == 0.0 { mid.checked_add(1).map(ordered) } else { Some(above) };
    below && above.is_none_or(|v| f64::from(v) >= cutoff)
}

fn first_nonnegative(volts: impl Fn(u16) -> f32) -> u16 {
    let (mut left, mut right) = (0, u16::MAX);
    while left < right {
        let mid = left + (right - left) / 2;
        if volts(mid) < 0.0 {
            left = mid + 1;
        } else {
            right = mid;
        }
    }
    left
}
