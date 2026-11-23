// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

/// The level crossing of the edge nearest to `pre` within +-`span` samples, as a fractional sample index (rising:
/// below -> at or above `level`; falling the other way). `either`: both directions. `hyst` > 0: like the hardware
/// comparators, only the first crossing after the signal was `hyst` on the other side of the level counts, so noise on
/// an edge of the other direction is no edge.
pub fn edge_near(v: &[f32], pre: usize, level: f32, hyst: f32, rising: bool, either: bool, span: usize) -> Option<f64> {
    if v.len() < 2 {
        return None;
    }
    let lo = pre.saturating_sub(span).max(1);
    let hi = (pre + span).min(v.len() - 1);
    // back from the crossing at k: armed when the signal went past the band before it came back over the level
    let look = span.max(4096);
    let armed = |k: usize, up: bool| {
        if hyst <= 0.0 {
            return true;
        }
        for j in (k.saturating_sub(look)..k).rev() {
            let x = v[j];
            if up && x < level - hyst || !up && x > level + hyst {
                return true;
            }
            if up && x >= level || !up && x <= level {
                return false;
            }
        }
        false
    };
    let mut best: Option<f64> = None;
    for k in lo..=hi {
        let (a, b) = (v[k - 1], v[k]);
        let up = a < level && b >= level && (rising || either) && armed(k, true);
        let down = a > level && b <= level && (!rising || either) && armed(k, false);
        if up || down {
            let t = (k - 1) as f64 + ((level - a) / (b - a)) as f64;
            if best.is_none_or(|x| (t - pre as f64).abs() < (x - pre as f64).abs()) {
                best = Some(t);
            }
        }
    }
    best
}

#[derive(Clone, Copy)]
pub(crate) struct EdgeSearch {
    pub(crate) level: f32,
    pub(crate) hysteresis: f32,
    pub(crate) rising: bool,
    pub(crate) either: bool,
}

/// Where the edge of the set direction is, seen at the resolution of the screen: `v` around `pre` (+-`span` samples)
/// smoothed by a centred moving average of `l` samples (one screen column), then the hardware's comparators with
/// hysteresis `hyst` (`oos_dsp::edges::trigger_edges`: a rising edge arms below level - `hyst` and fires at the level),
/// the edge of the set direction nearest to `pre`. Noise larger than the band
/// (a slow sine at a high rate) cannot make an edge of the other direction look right, which a test on single samples
/// cannot rule out. Only edges at times `fits` accepts count. Returns the edge and the nearest edge of either direction
/// (for the statistics).
pub(crate) fn edge_smooth<S: crate::trignear::TriggerSamples + ?Sized>(
    v: &S,
    pre: usize,
    trigger: EdgeSearch,
    span: usize,
    l: usize,
    fits: impl Fn(f64) -> bool,
) -> (Option<f64>, Option<oos_dsp::edges::Edge>) {
    crate::trignear::smooth_all(v, pre, trigger, span, l, &fits)
}

#[cfg(test)]
mod tests;
