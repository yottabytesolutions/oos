// SPDX-License-Identifier: GPL-2.0-or-later

//! Narrow-first trigger refinement (fork, docs/fork/perf-plan.md item 3). `fastdraw::edge_smooth` smooths and scans up
//! to half a screen on each side of the hardware trigger point for every frame, although the hardware usually put the
//! edge within a sample or two of it. `edge_smooth_near` first searches `NEAR_COLS` screen columns around the point and
//! keeps that answer only when it provably equals the wide one; otherwise it runs the wide search.
//!
//! In decoded samples, a search with half-span `s` and smoothing width `l` can move placement by up to
//! `s + ceil(l / 2)`: the search reads an extra `l` samples at each end and centers the averages.
//! The full-rate anchor refers to the trigger sample or its min/max pair (trigger.md §7).
//! Decimation can hide that crossing altogether. The legacy wide search can select a different edge
//! anywhere in its span, so sample or pair quantization alone cannot bound its result.
//!
//! Why the near answer is the wide one when it is accepted: `oos_dsp::edges::trigger_edges` keeps one armed flag per
//! direction. A sample below level - h arms the rising comparator and disarms the falling one (it is below the level);
//! a sample above level + h does the reverse. So both flags after such an out-of-band sample are the same whatever came
//! before, and after the first out-of-band sample in the near window both windows see the same states and the same
//! edges, each decided at its own crossing. Let d be the distance of the near result from `pre`. When the near window
//! has an out-of-band sample before `pre` - d, every wide edge within d is also a near edge, in the same order: the
//! nearest one (and the nearest of either direction) is the same. A finite smoothed sample after
//! `pre` + d bounds the interpolation of any crossing decided later. A second rail visit is unnecessary.

use crate::fastdraw::{Codes, EdgeSearch, Samples};
use crate::picture::deposit::kernel::SamplesRef;
use oos_dsp::edges::Edge;
use std::ops::Range;

mod pruned;
mod wide;
pub(crate) use wide::{WideSearch, WideShard};

pub(crate) type SearchResult = (Option<f64>, Option<Edge>);
pub(crate) type WideRefine<'a> = dyn FnMut(WideSearch, &dyn Fn(f64) -> bool) -> SearchResult + 'a;

/// Calibrated bounds that also enclose the shared one-sample smoothing pass.
pub(crate) fn code_trigger_bounds(codes: &Codes<'_>) -> Option<(f32, f32)> {
    pruned::bounds_for_single_sample_smoothing(codes)
}

pub(crate) trait TriggerSamples: Samples {
    fn trigger_codes(&self) -> Option<&Codes<'_>> {
        None
    }
}

impl TriggerSamples for [f32] {}
impl TriggerSamples for Vec<f32> {}
impl TriggerSamples for Codes<'_> {
    fn trigger_codes(&self) -> Option<&Codes<'_>> {
        Some(self)
    }
}
impl TriggerSamples for SamplesRef<'_> {
    fn trigger_codes(&self) -> Option<&Codes<'_>> {
        match self {
            Self::Codes(codes) => Some(codes),
            Self::Volts(_) => None,
        }
    }
}

/// Half width of the near search in screen columns (plus `NEAR_PAD` samples).
pub const NEAR_COLS: usize = 16;
pub const NEAR_PAD: usize = 64;

#[cfg(test)]
/// `fastdraw::edge_smooth` with the same arguments and the same result, searched near `pre` first.
#[allow(clippy::too_many_arguments)] // Keep the existing trigger refinement interface.
pub(crate) fn edge_smooth_near<S: TriggerSamples + ?Sized>(
    v: &S,
    pre: usize,
    level: f32,
    hyst: f32,
    rising: bool,
    either: bool,
    span: usize,
    l: usize,
    fits: impl Fn(f64) -> bool,
) -> SearchResult {
    edge_smooth_near_with(v, (pre as f64, span, l), EdgeSearch { level, hysteresis: hyst, rising, either }, &fits, None)
}

pub(crate) fn edge_smooth_near_with<S: TriggerSamples + ?Sized>(
    v: &S,
    (pre, span, l): (f64, usize, usize),
    trigger: EdgeSearch,
    fits: &dyn Fn(f64) -> bool,
    wide: Option<&mut WideRefine<'_>>,
) -> SearchResult {
    #[cfg(test)]
    if fixture::bypass_sparse() {
        return (Some(pre), None);
    }
    let l = l.max(1);
    let near = near_span(l);
    let bounds = sample_bounds(v.len(), (pre as usize, span, l));
    let local = if near >= span || bounds == sample_bounds(v.len(), (pre as usize, near, l)) {
        Some(smooth_all(v, pre, trigger, span, l, fits))
    } else {
        near_proof(v, (0, v.len()), (pre, near, l), trigger, fits, bounds.map(|b| b.start))
    };
    let result = local.unwrap_or_else(|| {
        crate::drawpipe::counters().routes.wide_search();
        if let Some(wide) = wide {
            wide(WideSearch::new(pre, span, l, trigger), fits)
        } else {
            smooth_all(v, pre, trigger, span, l, fits)
        }
    });
    #[cfg(test)]
    if pre.fract() == 0.0 {
        fixture::compare(v, (pre as usize, span, l), trigger, fits, fixture::Outcome::Refined(local.is_some(), result));
    }
    result
}

/// Samples `edge_near_window` needs of a frame of `len` samples: `near_window(pre, len, l)` = start..end.
#[cfg(test)]
pub fn near_window(pre: usize, len: usize, l: usize) -> (usize, usize) {
    let l = l.max(1);
    let near = near_span(l);
    (pre.saturating_sub(near + l), (pre + near + l).min(len))
}

/// Half-span of trigger refinement for a smoothing width of `l` played points.
pub(crate) fn near_span(l: usize) -> usize {
    NEAR_COLS * l + NEAR_PAD
}

/// The near search of `edge_smooth_near` alone, on a window of the samples: `v` holds samples `base..` of a frame of
/// `len` samples and covers `near_window`. Some where `edge_smooth_near` answers from the near search (the same
/// answer, the same arithmetic), None where it runs the wide search (then call it on all samples).
#[allow(clippy::too_many_arguments)] // The frame and the borrowed window have separate coordinates.
pub(crate) fn edge_near_window<S: TriggerSamples + ?Sized>(
    v: &S,
    base: usize,
    len: usize,
    pre: usize,
    level: f32,
    hyst: f32,
    rising: bool,
    either: bool,
    span: usize,
    l: usize,
    fits: impl Fn(f64) -> bool,
) -> Option<(Option<f64>, Option<Edge>)> {
    let l = l.max(1);
    let near = near_span(l);
    if near >= span {
        return None;
    }
    let trigger = EdgeSearch { level, hysteresis: hyst, rising, either };
    near_proof(v, (base, len), (pre as f64, near, l), trigger, &fits, None)
}

/// The near search: Some only when its answer is the wide search's (module comment). `v` holds samples `base..` of
/// `len`.
#[cfg(test)]
fn near_only<S: TriggerSamples + ?Sized>(
    v: &S,
    source: (usize, usize),
    geometry: (usize, usize, usize),
    trigger: EdgeSearch,
    fits: &dyn Fn(f64) -> bool,
) -> Option<(Option<f64>, Option<Edge>)> {
    near_proof(v, source, (geometry.0 as f64, geometry.1, geometry.2), trigger, fits, None)
}

fn near_proof<S: TriggerSamples + ?Sized>(
    v: &S,
    source: (usize, usize),
    (pre, span, l): (f64, usize, usize),
    trigger: EdgeSearch,
    fits: &dyn Fn(f64) -> bool,
    wide_origin: Option<usize>,
) -> Option<SearchResult> {
    let (level, hyst) = (trigger.level, trigger.hysteresis);
    search(v, source, (pre as usize, span, l), trigger, wide_origin, |a, origin, scratch| {
        let (time, any) = scratch.nearest(origin, pre, trigger, fits);
        let dist = (time? - pre).abs();
        let (first, last) = scratch.proof_bounds(level - hyst, level + hyst, wide_origin.is_none())?;
        let first = (a + first) as f64;
        let last = (a + last) as f64;
        (first < pre - dist && last > pre + dist).then_some((time, any))
    })
    .flatten()
}

/// The edge of `edges` (times from sample `a`) nearest `pre` that `ok` and `fits` accept, its time still from `a`.
fn nearest(edges: &[Edge], a: usize, pre: f64, ok: &dyn Fn(&Edge) -> bool, fits: &dyn Fn(f64) -> bool) -> Option<Edge> {
    let d = |e: &Edge| (a as f64 + e.t - pre).abs();
    edges.iter().filter(|e| ok(e) && fits(a as f64 + e.t)).min_by(|p, q| d(p).total_cmp(&d(q))).copied()
}

/// The refinement of a filtered trigger path (AC or LF-reject coupling, docs/fork/ac-trig.md): the edge nearest `pre`
/// within the near window only (`near_window`: `NEAR_COLS` columns plus `NEAR_PAD` samples, at most `span`), None
/// without one, never the wide search. The software level (frame mean) only approximates the high-pass the hardware
/// comparators see, so an edge farther away is a crossing the hardware did not trigger on; the hardware point stands.
/// `v` holds samples `base..` of a frame of `len` samples and covers `near_window(pre, len, l)`. Returns the edge and
/// the nearest edge of either direction, as `edge_smooth`.
#[cfg(test)]
#[allow(clippy::too_many_arguments)] // The frame and the borrowed window have separate coordinates.
pub(crate) fn edge_near<S: TriggerSamples + ?Sized>(
    v: &S,
    base: usize,
    len: usize,
    pre: usize,
    level: f32,
    hyst: f32,
    rising: bool,
    either: bool,
    span: usize,
    l: usize,
) -> (Option<f64>, Option<Edge>) {
    edge_near_from(v, (base, len), (pre as f64, span, l), EdgeSearch { level, hysteresis: hyst, rising, either })
}

/// Find a local crossing from the fractional anchor without searching the rest of the record.
pub(crate) fn edge_near_from<S: TriggerSamples + ?Sized>(
    v: &S,
    (base, len): (usize, usize),
    (pre, span, l): (f64, usize, usize),
    trigger: EdgeSearch,
) -> SearchResult {
    let l = l.max(1);
    let near = near_span(l).min(span);
    let fits = &|_| true;
    let result = search(v, (base, len), (pre as usize, near, l), trigger, None, |_, a, scratch| {
        scratch.nearest(a, pre, trigger, fits)
    })
    .unwrap_or_default();
    #[cfg(test)]
    if base == 0 && len == v.len() && pre.fract() == 0.0 {
        fixture::compare(v, (pre as usize, span, l), trigger, fits, fixture::Outcome::Anchored(result));
    }
    result
}

/// The decoded grid can smooth over fewer points than the plot used to size a crop.
/// Use the crop's spare points before the screen and anchor to arm the comparator,
/// and accept crossings only within the decoded near window.
pub(crate) fn edge_near_cropped<S: TriggerSamples + ?Sized>(
    v: &S,
    (pre, span, l): (f64, usize, usize),
    screen_first: f64,
    trigger: EdgeSearch,
) -> Option<f64> {
    let l = l.max(1);
    let index = pre as usize;
    let bounds = sample_bounds(v.len(), (index, near_span(l).min(span), l))?;
    let first_edge = (bounds.start + l / 2) as f64;
    let padding = screen_first.min(pre) as usize;
    let history = padding.saturating_sub(oos_dsp::interp::LANCZOS8_RADIUS).min(span + l);
    let start = bounds.start.min(index.saturating_sub(history));
    search_in(v, 0, (start..bounds.end, l), trigger, None, |_, a, scratch| {
        scratch.nearest(a, pre, trigger, &|ti| ti >= first_edge).0
    })
}

pub(crate) fn smooth_all<S: TriggerSamples + ?Sized>(
    v: &S,
    pre: f64,
    trigger: EdgeSearch,
    span: usize,
    l: usize,
    fits: &dyn Fn(f64) -> bool,
) -> (Option<f64>, Option<Edge>) {
    let l = l.max(1);
    let index = pre as usize;
    let origin = Some(index.saturating_sub(span + l));
    search(v, (0, v.len()), (index, span, l), trigger, origin, |_, a, scratch| scratch.nearest(a, pre, trigger, fits))
        .unwrap_or_default()
}

#[derive(Default)]
pub(crate) struct SearchScratch {
    samples: Vec<f32>,
    indexed: Vec<(usize, f32)>,
    edges: Vec<Edge>,
    window: Vec<f64>,
}

impl SearchScratch {
    fn proof_bounds(&self, lo: f32, hi: f32, require_end_rail: bool) -> Option<(usize, usize)> {
        let outside = |x: f32| x.is_finite() && (x > hi || x < lo);
        let end = |x: f32| x.is_finite() && (!require_end_rail || outside(x));
        if self.indexed.is_empty() {
            Some((self.samples.iter().position(|&x| outside(x))?, self.samples.iter().rposition(|&x| end(x))?))
        } else {
            Some((self.indexed.iter().find(|&&(_, x)| outside(x))?.0, self.indexed.iter().rfind(|&&(_, x)| end(x))?.0))
        }
    }

    fn nearest(
        &self,
        a: usize,
        pre: f64,
        trigger: EdgeSearch,
        fits: &dyn Fn(f64) -> bool,
    ) -> (Option<f64>, Option<Edge>) {
        let want = nearest(&self.edges, a, pre, &|e| trigger.either || e.rising == trigger.rising, fits);
        let any = nearest(&self.edges, a, pre, &|_| true, fits);
        (want.map(|e| a as f64 + e.t), any.map(|e| Edge { t: a as f64 + e.t, ..e }))
    }
}

// The buffers belong to the calling worker. The smoothing keeps the original sum of rounded f32 volts.
fn search<S: TriggerSamples + ?Sized, T>(
    v: &S,
    source: (usize, usize),
    geometry: (usize, usize, usize),
    trigger: EdgeSearch,
    interpolation_origin: Option<usize>,
    select: impl FnOnce(usize, usize, &SearchScratch) -> T,
) -> Option<T> {
    let bounds = sample_bounds(source.1, geometry)?;
    Some(search_in(v, source.0, (bounds, geometry.2), trigger, interpolation_origin, select))
}

fn search_in<S: TriggerSamples + ?Sized, T>(
    v: &S,
    base: usize,
    (bounds, l): (Range<usize>, usize),
    trigger: EdgeSearch,
    interpolation_origin: Option<usize>,
    select: impl FnOnce(usize, usize, &SearchScratch) -> T,
) -> T {
    crate::fastdraw::with_scratch(|work| {
        let scratch = &mut work.trigger;
        let SearchScratch { samples, indexed, edges, window } = scratch;
        let a = bounds.start;
        // Use the wide origin for interpolation so adding the fractional index rounds the same way.
        let origin = interpolation_origin.unwrap_or(a);
        let offset = a - origin;
        let rebase = |(i, x)| (offset + i, x);
        indexed.clear();
        if let Some(codes) = v.trigger_codes().filter(|_| bounds.len() >= pruned::MIN_SAMPLES) {
            pruned::smooth(codes, base, bounds.clone(), l, trigger, (indexed, window));
        }
        if indexed.is_empty() {
            let initial_sum = if interpolation_origin.is_some() { 0.0 } else { -0.0 };
            smoothed(v, base, bounds, l, (trigger.level, initial_sum), (samples, window));
            oos_dsp::edges::trigger_edges_indexed_into(
                samples.iter().copied().enumerate().map(rebase),
                trigger.level,
                trigger.hysteresis,
                edges,
            );
        } else {
            oos_dsp::edges::trigger_edges_indexed_into(
                indexed.iter().copied().map(rebase),
                trigger.level,
                trigger.hysteresis,
                edges,
            );
        }
        select(a, origin, scratch)
    })
}

fn sample_bounds(len: usize, (pre, span, l): (usize, usize, usize)) -> Option<Range<usize>> {
    let a = pre.saturating_sub(span + l);
    let b = (pre + span + l).min(len);
    (b > a + 2 * l).then_some(a..b)
}

/// Centred moving averages, with NaN where the window is incomplete.
fn smoothed<S: Samples + ?Sized>(
    v: &S,
    base: usize,
    bounds: Range<usize>,
    l: usize,
    (level, initial_sum): (f32, f64),
    (s, window): (&mut Vec<f32>, &mut Vec<f64>),
) {
    let (a, b) = (bounds.start, bounds.end);
    let x = |i: usize| finite_or_level(v.at(i - base), level);
    let half = l / 2;
    s.clear();
    s.reserve(b - a);
    s.resize(half, f32::NAN);
    smooth_run(a..b - l + 1, l, window, x, |w| sum_window(w, initial_sum), |_, average| s.push(average));
    s.resize(b - a, f32::NAN);
}

/// Visit the averages of consecutive complete windows, given their left indices.
fn smooth_run(
    starts: Range<usize>,
    l: usize,
    window: &mut Vec<f64>,
    x: impl Fn(usize) -> f64,
    start_sum: impl FnOnce(&[f64]) -> f64,
    mut emit: impl FnMut(usize, f32),
) -> (f64, f64) {
    let a = starts.start;
    window.clear();
    window.extend((a..a + l).map(&x));
    let first_sum = start_sum(window);
    let mut sum = first_sum;
    let center = l / 2;
    let divisor = l as f64;
    emit(a + center, (sum / divisor) as f32);
    let mut first = 0;
    for start in a + 1..starts.end {
        let next = x(start + l - 1);
        sum += next - std::mem::replace(&mut window[first], next);
        first += 1;
        if first == l {
            first = 0;
        }
        emit(start + center, (sum / divisor) as f32);
    }
    (first_sum, sum)
}

fn sum_window(window: &[f64], initial: f64) -> f64 {
    window.iter().fold(initial, |sum, &value| sum + value)
}

fn finite_or_level(sample: f32, level: f32) -> f64 {
    f64::from(if sample.is_finite() { sample } else { level })
}

#[cfg(test)]
mod bench;
#[cfg(test)]
pub(crate) mod fixture;
#[cfg(test)]
mod reference;
#[cfg(test)]
mod tests {
    use super::*;
    use oos_dsp::sim;
    mod corpus;
    mod exact;
    pub(super) use corpus::corpus;

    /// Reference timing (run with --ignored --nocapture): near-first vs the wide search at 12.5 k and 100 k points with
    /// the edge at the hardware point.
    #[test]
    #[ignore = "host timing: set OOS_PERF_TESTS=1"]
    fn near_first_is_much_cheaper_at_the_hardware_point() {
        assert_eq!(std::env::var("OOS_PERF_TESTS").as_deref(), Ok("1"), "set OOS_PERF_TESTS=1");
        for (n, l) in [(12_500usize, 12usize), (100_000, 100)] {
            let p = sim::PulseTrain::square(1e6 / (n as f64 / 1000.0), -1.0, 1.0, 1e-9);
            let mut v = sim::sample(|t| p.v(t), -(n as f64) / 2.0 * 1e-9, 1e-9, n);
            sim::add_noise(&mut v, 0.05, 3);
            let pre = n / 2;
            let (span, reps) = (n / 2, 2000);
            let t0 = std::time::Instant::now();
            let mut acc = 0.0;
            for _ in 0..reps {
                acc += reference::search(
                    std::hint::black_box(v.as_slice()),
                    (0, n),
                    (pre, span, l),
                    crate::fastdraw::EdgeSearch { level: 0.0, hysteresis: 0.06, rising: true, either: false },
                    false,
                    |_| true,
                )
                .unwrap_or_default()
                .0
                .unwrap_or(0.0);
            }
            let wide = t0.elapsed();
            let t0 = std::time::Instant::now();
            for _ in 0..reps {
                acc -= edge_smooth_near(std::hint::black_box(&v), pre, 0.0, 0.06, true, false, span, l, |_| true)
                    .0
                    .unwrap_or(0.0);
            }
            let near = t0.elapsed();
            let r = near.as_secs_f64() / wide.as_secs_f64();
            println!(
                "{} points: wide {:?}, near-first {:?} per frame, ratio {:.3} ({})",
                n,
                wide / reps,
                near / reps,
                r,
                acc
            );
            assert!(r <= 0.2, "near/wide {:.3}", r);
        }
    }
}
