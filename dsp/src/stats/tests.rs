// SPDX-License-Identifier: GPL-2.0-or-later
//! The shared helpers against the copies they replaced, on random records with NaN, infinities, signed zeros and
//! repeated values. Results must be bit-equal.

use super::*;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: u64) -> usize {
        (self.next() % n) as usize
    }

    fn sample(&mut self) -> f32 {
        match self.below(20) {
            0 => f32::NAN,
            1 => f32::INFINITY,
            2 => f32::NEG_INFINITY,
            3 => 0.0,
            4 => -0.0,
            5 => 1.0,
            _ => (self.next() % 2001) as f32 / 100.0 - 10.0,
        }
    }

    fn record(&mut self) -> Vec<f32> {
        let n = self.below(12);
        (0..n).map(|_| self.sample()).collect()
    }

    fn index(&mut self, n: usize) -> f64 {
        match self.below(4) {
            0 => self.below(n as u64 + 2) as f64,
            _ => (self.next() % 1000) as f64 / 1000.0 * (n as f64 + 2.0) - 1.0,
        }
    }
}

fn same(a: f64, b: f64) -> bool {
    a.to_bits() == b.to_bits() || a.is_nan() && b.is_nan()
}

fn same_opt(a: Option<f64>, b: Option<f64>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => same(a, b),
        (None, None) => true,
        _ => false,
    }
}

const RUNS: usize = 20_000;

/// The median of `dpt`.
fn old_median_dpt(x: &[f32]) -> f64 {
    let mut v: Vec<f64> = x.iter().copied().filter(|z| z.is_finite()).map(|z| z as f64).collect();
    if v.is_empty() {
        return f64::NAN;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = v.len();
    if n % 2 == 1 {
        v[n / 2]
    } else {
        0.5 * (v[n / 2 - 1] + v[n / 2])
    }
}

/// The median of `measure`, which only ever sees finite samples.
fn old_median_measure(f: &[f32]) -> f64 {
    let mut s = f.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = s.len();
    if n == 0 {
        f64::NAN
    } else if n % 2 == 1 {
        s[n / 2] as f64
    } else {
        (s[n / 2 - 1] as f64 + s[n / 2] as f64) / 2.0
    }
}

fn old_mean_finite(x: &[f32]) -> Option<f64> {
    let (mut s, mut n) = (0.0, 0usize);
    for &v in x {
        if v.is_finite() {
            s += v as f64;
            n += 1;
        }
    }
    (n > 0).then_some(s / n as f64)
}

/// The crossing of `power` (on f32) and `dpt` (on f64, with an extra `q != p`).
fn old_crossing_f32(x: &[f32], a: usize, b: usize, level: f64, up: bool) -> Option<f64> {
    (a.max(1)..b.min(x.len())).find_map(|k| {
        let (p, q) = (x[k - 1] as f64, x[k] as f64);
        let hit = if up { p < level && q >= level } else { p > level && q <= level };
        (hit && p.is_finite() && q.is_finite()).then(|| (k - 1) as f64 + (level - p) / (q - p))
    })
}

fn old_crossing_f64(x: &[f64], a: usize, b: usize, level: f64, up: bool) -> Option<f64> {
    let b = b.min(x.len());
    (a.max(1)..b).find_map(|k| {
        let (p, q) = (x[k - 1], x[k]);
        let hit = if up { p < level && q >= level } else { p > level && q <= level };
        (hit && p.is_finite() && q.is_finite() && q != p).then(|| (k - 1) as f64 + (level - p) / (q - p))
    })
}

/// `at` of `dpt`.
fn old_at_dpt(x: &[f32], t: f64) -> f64 {
    if x.len() < 2 {
        return x.first().map(|v| *v as f64).unwrap_or(f64::NAN);
    }
    let k = (t.floor() as usize).min(x.len() - 2);
    let (p, q) = (x[k] as f64, x[k + 1] as f64);
    if p.is_finite() && q.is_finite() {
        p + (q - p) * (t - k as f64)
    } else {
        f64::NAN
    }
}

/// `at` of `magnetic`.
fn old_at_magnetic(x: &[f32], t: f64) -> f64 {
    if x.is_empty() {
        return f64::NAN;
    }
    let k = (t.floor() as usize).min(x.len() - 1);
    let v = x[k] as f64;
    if t.fract() == 0.0 || k + 1 >= x.len() {
        return v;
    }
    let w = x[k + 1] as f64;
    if v.is_finite() && w.is_finite() {
        v + (w - v) * (t - k as f64)
    } else {
        f64::NAN
    }
}

/// The line fit of `dpt::current_at`: the value at `t1`, a flat line when x does not spread.
fn old_line_at(pts: &[(f64, f64)], t1: f64) -> Option<f64> {
    if pts.len() < 2 {
        return None;
    }
    let m = pts.len() as f64;
    let (mt, my) = (pts.iter().map(|p| p.0).sum::<f64>() / m, pts.iter().map(|p| p.1).sum::<f64>() / m);
    let (mut num, mut den) = (0.0, 0.0);
    for &(t, y) in pts {
        num += (t - mt) * (y - my);
        den += (t - mt) * (t - mt);
    }
    let slope = if den > 0.0 { num / den } else { 0.0 };
    Some(my + slope * (t1 - mt))
}

/// `motor::slope_of` (threshold 1e-18) and `magnetic::lin_slope` (1e-30) on separate x and y.
fn old_slope(x: &[f64], y: &[f64], min_var: f64) -> Option<f64> {
    let n = x.len().min(y.len());
    if n < 2 {
        return None;
    }
    let (mx, my) = (x.iter().take(n).sum::<f64>() / n as f64, y.iter().take(n).sum::<f64>() / n as f64);
    let (mut var, mut cov) = (0.0, 0.0);
    for k in 0..n {
        let d = x[k] - mx;
        var += d * d;
        cov += d * (y[k] - my);
    }
    (var > min_var).then_some(cov / var)
}

/// `power::ls_slope` after its index bounds: running sums over the finite samples, time from the index.
fn old_ls_slope(x: &[f32], dt: f64) -> Option<f64> {
    let mut n = 0.0;
    let (mut st, mut sy) = (0.0, 0.0);
    for (i, &y) in x.iter().enumerate() {
        if y.is_finite() {
            st += i as f64 * dt;
            sy += y as f64;
            n += 1.0;
        }
    }
    if n < 2.0 {
        return None;
    }
    let (mt, my) = (st / n, sy / n);
    let (mut num, mut den) = (0.0, 0.0);
    for (i, &y) in x.iter().enumerate() {
        if y.is_finite() {
            let t = i as f64 * dt - mt;
            num += t * (y as f64 - my);
            den += t * t;
        }
    }
    (den > 0.0).then_some(num / den)
}

#[test]
fn line_fit_matches_the_old_fits() {
    let mut r = Rng(0x5bd1_e995_1b87_3593);
    for run in 0..RUNS {
        let x = r.record();
        // every fourth record is all zeros of one sign, so a different start of a sum shows in the sign of zero
        let x: Vec<f32> =
            if run % 4 == 0 { x.iter().map(|_| if run % 8 == 0 { -0.0 } else { 0.0 }).collect() } else { x };
        let finite: Vec<(f64, f64)> =
            x.iter().enumerate().filter(|(_, v)| v.is_finite()).map(|(i, &v)| (i as f64, v as f64)).collect();
        let (xs, ys): (Vec<f64>, Vec<f64>) = finite.iter().copied().unzip();
        let t1 = r.index(x.len());
        let fit = line_fit(finite.iter().copied());
        let at = fit.map(|f| f.mean_y + f.slope(0.0).unwrap_or(0.0) * (t1 - f.mean_x));
        assert!(same_opt(at, old_line_at(&finite, t1)), "{x:?}");
        for min in [1e-18, 1e-30] {
            assert!(same_opt(fit.and_then(|f| f.slope(min)), old_slope(&xs, &ys, min)), "{x:?}");
            let flipped = line_fit(finite.iter().map(|&(a, b)| (b, a))).and_then(|f| f.slope(min));
            assert!(same_opt(flipped, old_slope(&ys, &xs, min)), "{x:?}");
        }
        let dt = 1e-9 * (1 + r.below(1000)) as f64;
        let pts = x.iter().enumerate().filter(|(_, y)| y.is_finite()).map(|(i, &y)| (i as f64 * dt, y as f64));
        assert!(same_opt(line_fit(pts).and_then(|f| f.slope(0.0)), old_ls_slope(&x, dt)), "{x:?}");
    }
}

#[test]
fn median_matches_both_old_medians() {
    let mut r = Rng(0x9e37_79b9_7f4a_7c15);
    for _ in 0..RUNS {
        let x = r.record();
        assert!(same(median(&x), old_median_dpt(&x)), "{x:?}");
        let finite: Vec<f32> = x.iter().copied().filter(|v| v.is_finite()).collect();
        assert!(same(median(&finite), old_median_measure(&finite)), "{finite:?}");
    }
}

#[test]
fn mean_finite_matches_the_old_mean() {
    let mut r = Rng(0x2545_f491_4f6c_dd1d);
    for _ in 0..RUNS {
        let x = r.record();
        assert!(same_opt(mean_finite(&x), old_mean_finite(&x)), "{x:?}");
    }
}

#[test]
fn crossing_matches_both_old_crossings() {
    let mut r = Rng(0x1234_5678_9abc_def1);
    for _ in 0..RUNS {
        let x = r.record();
        let wide: Vec<f64> = x.iter().map(|&v| v as f64).collect();
        let (a, b) = (r.below(14), r.below(14));
        let level = r.sample() as f64;
        for up in [true, false] {
            let new = crossing(&x, a, b, level, up);
            assert!(same_opt(new, old_crossing_f32(&x, a, b, level, up)), "{x:?} {a} {b} {level} {up}");
            let new = crossing(&wide, a, b, level, up);
            assert!(same_opt(new, old_crossing_f64(&wide, a, b, level, up)), "{x:?} {a} {b} {level} {up}");
        }
    }
}

#[test]
fn values_at_fractional_indices_match_the_old_ones() {
    let mut r = Rng(0x0f0f_1e1e_2d2d_3c3c);
    for _ in 0..RUNS {
        let x = r.record();
        let t = r.index(x.len());
        assert!(same(at_extended(&x, t), old_at_dpt(&x, t)), "{x:?} {t}");
        assert!(same(at_held(&x, t), old_at_magnetic(&x, t)), "{x:?} {t}");
    }
}

#[test]
fn the_two_interpolations_differ_only_at_the_end_and_on_whole_indices() {
    let x = [0.0, 1.0, 3.0];
    assert_eq!(at_extended(&x, 2.5), 4.0);
    assert_eq!(at_held(&x, 2.5), 3.0);
    assert_eq!(at_extended(&x, 1.5), at_held(&x, 1.5));
    let gap = [1.0, f32::NAN];
    assert!(at_extended(&gap, 0.0).is_nan());
    assert_eq!(at_held(&gap, 0.0), 1.0);
}
