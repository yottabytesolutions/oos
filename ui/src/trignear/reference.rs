// SPDX-License-Identifier: GPL-2.0-or-later

use super::{nearest, Edge, EdgeSearch, Samples};

pub(super) fn search<S: Samples + ?Sized>(
    v: &S,
    source: (usize, usize),
    (pre, span, l): (usize, usize, usize),
    trigger: EdgeSearch,
    near: bool,
    fits: impl Fn(f64) -> bool,
) -> Option<(Option<f64>, Option<Edge>)> {
    crate::fastdraw::with_scratch(|scratch| {
        let super::SearchScratch { samples, edges, window, .. } = &mut scratch.trigger;
        let a = smoothed(v, source, (pre, span, l), (trigger.level, if near { -0.0 } else { 0.0 }), (samples, window))?;
        oos_dsp::edges::trigger_edges_into(samples, trigger.level, trigger.hysteresis, edges);
        let want = nearest(edges, a, pre as f64, &|e| trigger.either || e.rising == trigger.rising, &fits)
            .map(|e| a as f64 + e.t);
        let any = nearest(edges, a, pre as f64, &|_| true, &fits).map(|e| Edge { t: a as f64 + e.t, ..e });
        if near {
            let dist = (want? - pre as f64).abs();
            let (lo, hi) = (trigger.level - trigger.hysteresis, trigger.level + trigger.hysteresis);
            let outside = |x: &f32| x.is_finite() && (*x > hi || *x < lo);
            let first = samples.iter().position(outside)? as f64 + a as f64;
            let last = samples.iter().rposition(outside)? as f64 + a as f64;
            if !(first < pre as f64 - dist && last > pre as f64 + dist) {
                return None;
            }
        }
        Some((want, any))
    })
}

// The original running sum, including its first and last incomplete windows.
pub(super) fn smoothed<S: Samples + ?Sized>(
    v: &S,
    (base, len): (usize, usize),
    (pre, span, l): (usize, usize, usize),
    (level, initial_sum): (f32, f64),
    (s, window): (&mut Vec<f32>, &mut Vec<f64>),
) -> Option<usize> {
    let a = pre.saturating_sub(span + l);
    let b = (pre + span + l).min(len);
    if b <= a + 2 * l {
        return None;
    }
    let x = |i: usize| {
        let y = v.at(i - base);
        if y.is_finite() {
            y as f64
        } else {
            level as f64
        }
    };
    let half = l / 2;
    s.clear();
    s.reserve(b - a);
    window.clear();
    window.extend((a..a + l).map(x));
    let mut sum = window.iter().fold(initial_sum, |sum, &value| sum + value);
    let mut first = 0;
    for j in 0..(b - a) {
        let lo = a + j;
        if j >= half && lo + l - half <= b {
            s.push((sum / l as f64) as f32);
            if lo + l - half < b {
                let next = x(lo + l - half);
                sum += next - std::mem::replace(&mut window[first], next);
                first += 1;
                if first == l {
                    first = 0;
                }
            }
        } else {
            s.push(f32::NAN);
        }
    }
    Some(a)
}

/// Bitwise reference for the bounded proof, including sources whose moving sum depends on its start.
pub(super) fn bounded<S: Samples + ?Sized>(
    v: &S,
    (pre, span, width): (usize, usize, usize),
    trigger: EdgeSearch,
    fits: &dyn Fn(f64) -> bool,
) -> super::SearchResult {
    let wide = || search(v, (0, v.len()), (pre, span, width), trigger, false, fits).unwrap_or_default();
    let near = super::near_span(width);
    let origin = pre.saturating_sub(span + width);
    if near >= span {
        return wide();
    }
    let (mut samples, mut window) = (Vec::new(), Vec::new());
    let Some(a) = smoothed(v, (0, v.len()), (pre, near, width), (trigger.level, 0.0), (&mut samples, &mut window))
    else {
        return wide();
    };
    let mut edges = Vec::new();
    let indexed = samples.iter().enumerate().map(|(i, &x)| (a + i - origin, x));
    oos_dsp::edges::trigger_edges_indexed_into(indexed, trigger.level, trigger.hysteresis, &mut edges);
    let Some(want) = nearest(&edges, origin, pre as f64, &|e| trigger.either || e.rising == trigger.rising, fits)
    else {
        return wide();
    };
    let time = origin as f64 + want.t;
    let distance = (time - pre as f64).abs();
    let resets =
        |x: f32| x.is_finite() && (x < trigger.level - trigger.hysteresis || x > trigger.level + trigger.hysteresis);
    let past = samples.iter().copied().position(resets).is_some_and(|i| ((a + i) as f64) < pre as f64 - distance);
    let future = samples.iter().rposition(|x| x.is_finite()).is_some_and(|i| (a + i) as f64 > pre as f64 + distance);
    if past && future {
        let any = nearest(&edges, origin, pre as f64, &|_| true, fits).map(|e| Edge { t: origin as f64 + e.t, ..e });
        (Some(time), any)
    } else {
        wide()
    }
}
