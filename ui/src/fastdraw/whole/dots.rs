// SPDX-License-Identifier: GPL-2.0-or-later

use super::{min_max, Map, Range};
use crate::wave::RowBand;

/// Draw every sample as a pixel and return the exact hull of its on-map rows.
///
/// Where samples repeat the row before (flat or slow parts), one add per sample makes a chain of read-modify-writes on
/// one pixel, each waiting for the last; each run adds `run x wgt` at once, saturating as the single adds did.
/// On fast or noisy parts the run test would mispredict, so one add per sample stays. The choice follows how many
/// samples of the column before repeated their row; both give the same picture.
pub fn deposit_dots(map: &mut [u16], w: usize, h: usize, v: &[f32], m: &Map, wgt: u16) -> Range<usize> {
    let mut samples = RowBand::empty();
    dots_walk(w, v.len(), m, |_, a, b| samples.include(a..b));
    let samples = samples.into_range();
    if samples.is_empty() {
        return 0..0;
    }
    let Some(rows) = unclipped_rows(&v[samples], m, h) else {
        return deposit_dots_band(map, w, 0, h, v, m, wgt);
    };
    let mut runs = false;
    dots_walk(w, v.len(), m, |c, a, b| dots_column(map, w, h, c, &v[a..b], m, wgt, &mut runs));
    rows
}

fn unclipped_rows(samples: &[f32], mapping: &Map, height: usize) -> Option<Range<usize>> {
    // Avoid a whole-slice extrema pass when a small probe already proves the trace clips.
    if samples.iter().step_by((samples.len() / PROBE).max(1)).any(|&x| sample_row(x, mapping, height).is_none()) {
        return None;
    }
    let (low, high) = min_max(samples);
    let a = sample_row(low, mapping, height)?;
    let b = sample_row(high, mapping, height)?;
    Some(a.min(b)..a.max(b) + 1)
}

/// Scalar band drawing also tracks the exact hull when clipped extrema lie off the map.
pub fn deposit_dots_band(
    band: &mut [u16],
    w: usize,
    r_lo: usize,
    h: usize,
    v: &[f32],
    m: &Map,
    wgt: u16,
) -> Range<usize> {
    if v.is_empty() || m.x_per_sample <= 0.0 {
        return 0..0;
    }
    let r_hi = (r_lo + band.len() / w).min(h);
    let (mut first, mut end) = (r_hi, 0);
    dots_walk(w, v.len(), m, |c, a, b| {
        for &x in &v[a..b] {
            if let Some(r) = sample_row(x, m, r_hi).filter(|&r| r >= r_lo) {
                let i = (r - r_lo) * w + c;
                band[i] = band[i].saturating_add(wgt);
                first = first.min(r);
                end = end.max(r + 1);
            }
        }
    });
    first.min(end)..end
}

/// Visit each visible column and its contiguous sample range.
fn dots_walk(w: usize, n: usize, m: &Map, mut col: impl FnMut(usize, usize, usize)) {
    if n == 0 || m.x_per_sample <= 0.0 {
        return;
    }
    let k_of = |x: f64| m.ti + (x - m.x0) / m.x_per_sample;
    let first = |c: i64| (k_of(c as f64).ceil().max(0.0) as usize).min(n);
    let c0 = (((-m.ti) * m.x_per_sample + m.x0).floor() as i64).max(0);
    let c1 = ((((n as f64 - m.ti) * m.x_per_sample + m.x0).ceil()) as i64).min(w as i64);
    let mut ka = first(c0);
    for c in c0..c1 {
        let kb = first(c + 1).max(ka);
        if kb > ka {
            col(c as usize, ka, kb);
        }
        ka = kb;
    }
}

/// Use runs where rows repeat; probe the preceding column to select single adds elsewhere.
// The column kernel shares its map, mapping and run selection with the whole-map walk.
#[allow(clippy::too_many_arguments)]
#[inline]
fn dots_column(map: &mut [u16], w: usize, h: usize, c: usize, s: &[f32], m: &Map, wgt: u16, runs: &mut bool) {
    let row = |x| sample_row(x, m, h);
    let mut repeats = 0usize;
    if *runs {
        let mut run: (Option<usize>, u32) = (None, 0);
        for &x in s {
            let r = row(x);
            if r == run.0 {
                run.1 += 1;
                repeats += 1;
                continue;
            }
            if let Some(r0) = run.0 {
                let i = r0 * w + c;
                map[i] = (map[i] as u32 + run.1 * wgt as u32).min(u16::MAX as u32) as u16;
            }
            run = (r, 1);
        }
        if let Some(r0) = run.0 {
            let i = r0 * w + c;
            map[i] = (map[i] as u32 + run.1 * wgt as u32).min(u16::MAX as u32) as u16;
        }
        *runs = 2 * repeats > s.len();
    } else {
        // the repeats of the first `PROBE` samples decide the next column (counting all cost more than it saved)
        let (head, tail) = s.split_at(s.len().min(PROBE));
        let mut prev = None;
        for &x in head {
            let r = row(x);
            repeats += (r == prev) as usize;
            prev = r;
            if let Some(r) = r {
                let i = r * w + c;
                map[i] = map[i].saturating_add(wgt);
            }
        }
        for &x in tail {
            if let Some(r) = row(x) {
                let i = r * w + c;
                map[i] = map[i].saturating_add(wgt);
            }
        }
        *runs = 2 * repeats > head.len();
    }
}

#[inline]
fn sample_row(value: f32, mapping: &Map, height: usize) -> Option<usize> {
    let row = (value * mapping.y_per_volt + mapping.y0).round();
    if row >= 0.0 && row < height as f32 {
        let row = row as usize;
        Some(row)
    } else {
        None
    }
}

const PROBE: usize = 32;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_cover_dots_with_nan_gaps_and_clipping() {
        let m = Map { ti: 0.0, x_per_sample: 1.0, x0: 0.0, y_per_volt: -2.0, y0: 16.0 };
        for samples in [
            vec![],
            vec![8.0, -7.5],
            vec![0.0, 1.0, f32::NAN],
            vec![-100.0],
            vec![100.0],
            vec![f32::NAN],
            vec![f32::NEG_INFINITY, 0.0, f32::INFINITY],
            vec![-100.0, 0.0, 100.0],
            vec![0.0; 64],
        ] {
            for initial_runs in [false, true] {
                let mut runs = initial_runs;
                let mut map = vec![0; 32];
                dots_column(&mut map, 1, 32, 0, &samples, &m, 24, &mut runs);
                let mut bounded = vec![0; 32];
                let band = deposit_dots(
                    &mut bounded,
                    1,
                    32,
                    &samples,
                    &Map { x_per_sample: 1.0 / samples.len().max(1) as f64, ..m },
                    24,
                );
                assert_eq!(map, bounded);
                let rows: Vec<_> = map.iter().enumerate().filter(|(_, value)| **value != 0).map(|(y, _)| y).collect();
                let expected = rows.first().zip(rows.last()).map_or(0..0, |(&first, &last)| first..last + 1);
                assert_eq!(band, expected, "samples {samples:?}, runs {initial_runs}");
            }
        }
    }

    #[test]
    fn bounds_use_only_visible_samples_and_check_clipping_between_probes() {
        let m = Map { ti: 512.0, x_per_sample: 1.0 / 128.0, x0: 2.0, y_per_volt: -2.0, y0: 16.0 };
        let mut samples = vec![0.0; 1024];
        samples[0] = 100.0;
        samples[1023] = -100.0;
        samples[263] = 100.0;
        samples[289] = 7.5;
        let mut map = vec![0; 4 * 32];
        assert_eq!(deposit_dots(&mut map, 4, 32, &samples, &m, 24), 1..17);
        let mut reference = vec![0; 4 * 32];
        super::super::deposit_dots_reference(&mut reference, 4, 32, &samples, &m, 24);
        assert_eq!(map, reference);
    }

    #[test]
    fn empty_and_offscreen_samples_have_no_dirty_rows() {
        let m = Map { ti: 0.0, x_per_sample: 1.0, x0: 0.0, y_per_volt: 1.0, y0: 0.0 };
        for (w, h, samples, m) in [
            (0, 32, vec![0.0], m),
            (4, 0, vec![0.0], m),
            (4, 32, vec![], m),
            (4, 32, vec![0.0], Map { x0: 5.0, ..m }),
            (4, 32, vec![0.0], Map { x_per_sample: 0.0, ..m }),
        ] {
            let mut map = vec![0; w * h];
            assert_eq!(deposit_dots(&mut map, w, h, &samples, &m, 24), 0..0);
            assert!(map.iter().all(|&value| value == 0));
        }
    }
}
