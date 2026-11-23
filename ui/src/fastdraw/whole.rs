// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

use super::{code_rows, Codes, Map, Spans};
use std::ops::Range;

// Reused by the pool workers and the standalone drawing path on their own threads.
#[derive(Default)]
pub(crate) struct Scratch {
    spans: Spans,
    rows: code_rows::RowCache,
    pub(crate) routing: crate::drawpipe::route::Scratch,
    pub(crate) envelope: crate::picture::deposit::envelope::Scratch,
    pub(crate) trigger: crate::trignear::SearchScratch,
}

thread_local! {
    static SPANS: std::cell::RefCell<Scratch> = std::cell::RefCell::new(Scratch::default());
}

pub(crate) fn with_scratch<T>(draw: impl FnOnce(&mut Scratch) -> T) -> T {
    SPANS.with(|scratch| draw(&mut scratch.borrow_mut()))
}

mod dots;
#[cfg(test)]
mod work;
pub use dots::deposit_dots;
#[cfg(test)]
pub fn deposit_dots_band(band: &mut [u16], w: usize, r_lo: usize, h: usize, v: &[f32], m: &Map, wgt: u16) {
    let _ = dots::deposit_dots_band(band, w, r_lo, h, v, m, wgt);
}
/// Adds `wgt` to every pixel of `map` (w x h, row-major) the trace covers: per column the span of its samples, joined
/// to the last sample of the column before (so steep edges stay connected). Needs at least one sample per column
/// (`x_per_sample <= 1`).
///
/// The spans are worked out first (`spans`) and then added in one of two ways with the same result: column by column
/// (one scattered read-modify-write per pixel, a row apart in memory) when the spans are short, or row by row over the
/// rows the frame touches (a vectorised pass that adds `wgt` where the row lies in a column's span) when they are long:
/// a noisy or fast signal at 100 samples per column covers tens of rows in every column, and the scattered writes
/// were three quarters of the time.
#[must_use]
pub fn deposit(map: &mut [u16], w: usize, h: usize, v: &[f32], m: &Map, wgt: u16) -> Range<usize> {
    deposit_samples::<false>(map, w, h, v, m, wgt)
}

fn deposit_samples<const TRACK: bool>(
    map: &mut [u16],
    w: usize,
    h: usize,
    v: &(impl super::Samples + ?Sized),
    m: &Map,
    wgt: u16,
) -> Range<usize> {
    deposit_from::<TRACK>(map, w, wgt, |scratch| super::spans_into(w, h, v, m, &mut scratch.spans))
}
/// `deposit` straight from a frame's read-out words (`Codes`): the same picture, without the f32 copy of the channel.
#[must_use]
pub fn deposit_codes(map: &mut [u16], w: usize, h: usize, v: &Codes, m: &Map, wgt: u16) -> Range<usize> {
    deposit_from::<false>(map, w, wgt, |scratch| {
        super::code_spans_into(w, h, v, m, &mut scratch.rows, &mut scratch.spans)
    })
}

fn deposit_from<const TRACK: bool>(
    map: &mut [u16],
    w: usize,
    wgt: u16,
    build: impl FnOnce(&mut Scratch),
) -> Range<usize> {
    super::with_scratch(|scratch| {
        build(scratch);
        let sp = &scratch.spans;
        let Some((c0, c1, rmin, rmax, pixels)) = span_extent(sp) else { return 0..0 };
        if pixels > sweep_cost(rmax - rmin + 1, c1 - c0) {
            for r in rmin..=rmax {
                let row = &mut map[r * w + c0..r * w + c1];
                let r = r as i16;
                #[cfg(test)]
                if TRACK {
                    work::spans(0, row.len());
                }
                for (px, &(a, b)) in row.iter_mut().zip(&sp.rows) {
                    let inside = ((a <= r) & (r <= b)) as u16;
                    *px = px.saturating_add(wgt & 0u16.wrapping_sub(inside));
                }
            }
        } else {
            for (c, &(a, b)) in sp.rows.iter().enumerate() {
                let mut i = a.max(0) as usize * w + c0 + c;
                #[cfg(test)]
                if TRACK {
                    work::spans((a..=b).len(), 0);
                }
                for _ in a..=b {
                    map[i] = map[i].saturating_add(wgt);
                    i += w;
                }
            }
        }
        rmin..rmax + 1
    })
}

fn span_extent(sp: &Spans) -> Option<(usize, usize, usize, usize, usize)> {
    let (rmin, end) = super::span_rows(sp);
    let pixels: usize = sp.rows.iter().filter(|(a, b)| a <= b).map(|&(a, b)| (b - a + 1) as usize).sum();
    (pixels > 0).then_some((sp.c0, sp.c0 + sp.rows.len(), rmin, end.saturating_sub(1), pixels))
}

/// Scattered pixel writes the row sweep of `rows` x `cols` pixels is worth: about one scattered read-modify-write per
/// eight pixels of the sweep (eight u16 lanes per vector step, a compare and an add each). Only the speed depends on it.
fn sweep_cost(rows: usize, cols: usize) -> usize {
    rows * cols / 8
}

/// `deposit` as it was before the span pass (one scattered write per pixel, one compare chain per sample): the
/// reference for equality and the work budget.
#[cfg(test)]
pub fn deposit_reference(map: &mut [u16], w: usize, h: usize, v: &[f32], m: &Map, wgt: u16) {
    if v.is_empty() || m.x_per_sample <= 0.0 {
        return;
    }
    let k_of = |x: f64| m.ti + (x - m.x0) / m.x_per_sample;
    let first = |c: i64| (k_of(c as f64).ceil().max(0.0) as usize).min(v.len());
    let c0 = (((-m.ti) * m.x_per_sample + m.x0).floor() as i64).max(0);
    let c1 = ((((v.len() as f64 - m.ti) * m.x_per_sample + m.x0).ceil()) as i64).min(w as i64);
    let hh = h as i32;
    let mut ka = first(c0);
    for c in c0..c1 {
        let kb = first(c + 1);
        if kb <= ka {
            continue;
        }
        let (mut mn, mut mx) = super::samples::min_max_scalar(&v[ka..kb]);
        if ka > 0 && v[ka - 1].is_finite() {
            mn = mn.min(v[ka - 1]);
            mx = mx.max(v[ka - 1]);
        }
        ka = kb;
        if mn.partial_cmp(&mx).is_none_or(|order| order.is_gt()) {
            continue;
        }
        let (ya, yb) = (mn * m.y_per_volt + m.y0, mx * m.y_per_volt + m.y0);
        let r0 = (ya.min(yb).round() as i32).max(0);
        let r1 = (ya.max(yb).round() as i32).min(hh - 1);
        let mut i = r0.max(0) as usize * w + c as usize;
        for _ in r0..=r1 {
            map[i] = map[i].saturating_add(wgt);
            i += w;
        }
    }
}

/// `deposit_dots` as it was (one scattered write per sample): the reference for equality and work.
#[cfg(test)]
pub fn deposit_dots_reference(map: &mut [u16], w: usize, h: usize, v: &[f32], m: &Map, wgt: u16) {
    if v.is_empty() || m.x_per_sample <= 0.0 {
        return;
    }
    let k_of = |x: f64| m.ti + (x - m.x0) / m.x_per_sample;
    let first = |c: i64| (k_of(c as f64).ceil().max(0.0) as usize).min(v.len());
    let c0 = (((-m.ti) * m.x_per_sample + m.x0).floor() as i64).max(0);
    let c1 = ((((v.len() as f64 - m.ti) * m.x_per_sample + m.x0).ceil()) as i64).min(w as i64);
    let mut ka = first(c0);
    for c in c0..c1 {
        let kb = first(c + 1);
        for &x in &v[ka..kb.max(ka)] {
            let r = (x * m.y_per_volt + m.y0).round();
            if r >= 0.0 && r < h as f32 {
                let i = r as usize * w + c as usize;
                map[i] = map[i].saturating_add(wgt);
            }
        }
        ka = kb.max(ka);
    }
}

#[cfg(test)]
mod tests;
