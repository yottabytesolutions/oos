// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! Numeric helpers on sampled waveforms shared by the measurement modules: the median and mean of the finite
//! samples, a least-squares line, level crossings and values at fractional sample indices.

use std::cmp::Ordering;

/// Median of the finite samples (the mean of the two middle ones for an even count); NaN without any.
pub(crate) fn median(x: &[f32]) -> f64 {
    let mut v: Vec<f32> = x.iter().copied().filter(|z| z.is_finite()).collect();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
    let n = v.len();
    match n {
        0 => f64::NAN,
        _ if n % 2 == 1 => v[n / 2] as f64,
        _ => 0.5 * (v[n / 2 - 1] as f64 + v[n / 2] as f64),
    }
}

/// Mean of the finite samples; None without any.
pub(crate) fn mean_finite(x: &[f32]) -> Option<f64> {
    let (s, n) = x.iter().filter(|v| v.is_finite()).fold((0.0, 0usize), |(s, n), &v| (s + v as f64, n + 1));
    (n > 0).then_some(s / n as f64)
}

/// Least-squares line through points (x, y): the means and the sums of the centred products.
#[derive(Clone, Copy, Debug)]
pub(crate) struct LineFit {
    pub(crate) mean_x: f64,
    pub(crate) mean_y: f64,
    sxx: f64,
    sxy: f64,
}

impl LineFit {
    /// The slope, when the spread of x is above `min_sxx`.
    pub(crate) fn slope(&self, min_sxx: f64) -> Option<f64> {
        (self.sxx > min_sxx).then(|| self.sxy / self.sxx)
    }
}

/// Fit through at least two points; None with fewer.
pub(crate) fn line_fit<I: Iterator<Item = (f64, f64)> + Clone>(pts: I) -> Option<LineFit> {
    let n = pts.clone().count();
    if n < 2 {
        return None;
    }
    let mean_x = pts.clone().map(|p| p.0).sum::<f64>() / n as f64;
    let mean_y = pts.clone().map(|p| p.1).sum::<f64>() / n as f64;
    let (sxx, sxy) = pts.fold((0.0, 0.0), |(sxx, sxy), (x, y)| {
        let d = x - mean_x;
        (sxx + d * d, sxy + d * (y - mean_y))
    });
    Some(LineFit { mean_x, mean_y, sxx, sxy })
}

/// First crossing of `level` in direction `up` within samples [a, b) (fractional index).
pub(crate) fn crossing<T: Copy + Into<f64>>(x: &[T], a: usize, b: usize, level: f64, up: bool) -> Option<f64> {
    (a.max(1)..b.min(x.len())).find_map(|k| {
        let (p, q) = (x[k - 1].into(), x[k].into());
        let hit = if up { p < level && q >= level } else { p > level && q <= level };
        (hit && p.is_finite() && q.is_finite()).then(|| (k - 1) as f64 + (level - p) / (q - p))
    })
}

/// Value at fractional index `t`, linear between the samples; beyond the ends the first or last segment is extended.
/// NaN next to a non-finite sample.
pub(crate) fn at_extended(x: &[f32], t: f64) -> f64 {
    if x.len() < 2 {
        return x.first().map_or(f64::NAN, |&v| v as f64);
    }
    between(x, (t.floor() as usize).min(x.len() - 2), t)
}

/// Value at fractional index `t`, linear between the samples; a whole index and anything from the last sample on
/// give that sample as it is.
pub(crate) fn at_held(x: &[f32], t: f64) -> f64 {
    if x.is_empty() {
        return f64::NAN;
    }
    let k = (t.floor() as usize).min(x.len() - 1);
    if t.fract() == 0.0 || k + 1 >= x.len() {
        return x[k] as f64;
    }
    between(x, k, t)
}

/// Linear between samples `k` and `k + 1` at `t`; NaN when either is not finite.
fn between(x: &[f32], k: usize, t: f64) -> f64 {
    let (p, q) = (x[k] as f64, x[k + 1] as f64);
    if p.is_finite() && q.is_finite() {
        p + (q - p) * (t - k as f64)
    } else {
        f64::NAN
    }
}

#[cfg(test)]
mod tests;
