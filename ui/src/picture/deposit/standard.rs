// SPDX-License-Identifier: GPL-2.0-or-later

//! Adapters from the existing Standard and Batch routes to frozen kernel inputs.

use super::kernel::{self, FrameTiming, Placed, Projection, RawTrace, SamplesRef, TrigSnapshot, Vertical};
use crate::model::*;

pub(crate) fn projection(m: &Model, w: usize, h: usize, x_per_sample: f64) -> Projection {
    Projection {
        cols: w,
        rows: h,
        x_per_sample,
        time_per_div: m.time_per_div() as f64,
        delay: m.delay as f64,
        ch: std::array::from_fn(|c| {
            m.ch[c].on.then(|| Vertical { volts_per_div: m.ch[c].volts_per_div(), offset: m.ch[c].offset })
        }),
        dots: m.dots_at(x_per_sample),
    }
}

pub(crate) fn frame_timing(m: &Model, f: &crate::board::Frame) -> FrameTiming {
    FrameTiming {
        dt: f.dt,
        pre: f.pre,
        trigger: TrigSnapshot {
            source: m.trig_src,
            hysteresis: m.trig_hyst_div() * m.ch[m.trig_src].volts_per_div(),
            rising: m.slope != Slope::Fall,
            either: m.slope == Slope::Either,
            on_la: m.trig_on_la(),
            filtered: m.trig_coupling() != TrigCoup::Dc,
        },
    }
}

enum FastRoute {
    Rec(crate::wave::Record),
    Raw(RawTrace),
}

pub(crate) struct FastPlan {
    route: FastRoute,
    projection: Projection,
    fold: [Option<crate::fastdraw::Fold>; NCH],
}

impl FastPlan {
    pub(crate) fn fold(&self, c: usize) -> Option<crate::fastdraw::Fold> {
        self.fold[c]
    }

    pub(crate) fn outcome(&self) -> Option<&kernel::TrigOutcome> {
        match &self.route {
            FastRoute::Raw(p) => Some(&p.outcome),
            FastRoute::Rec(_) => None,
        }
    }

    pub(crate) fn rows(&self, c: usize) -> (usize, usize) {
        match &self.route {
            FastRoute::Raw(p) => p.spans[c].as_ref().map_or((0, self.projection.rows), crate::fastdraw::span_rows),
            FastRoute::Rec(rec) => self.projection.record_rows(c, &rec.ch[c]),
        }
    }

    fn trace_record(
        &self,
        c: usize,
        rec: &crate::wave::Record,
        columns: (i32, i32),
        put: impl FnMut(i32, i32, i32, u16),
    ) {
        let proj = &self.projection;
        let Some(ch) = proj.ch[c] else { return };
        crate::wave::trace_columns(
            proj.cols,
            proj.rows,
            (ch.volts_per_div, ch.offset),
            proj.dots,
            (proj.time_per_div, proj.delay),
            rec.t0,
            rec.dt,
            &rec.ch[c],
            24,
            None,
            rec.pairs,
            columns,
            put,
        );
    }
}

pub(crate) struct FastFrame {
    pub(crate) plan: FastPlan,
    pub(crate) triggered: bool,
}

pub(crate) fn fast_plan(
    m: &Model,
    w: usize,
    h: usize,
    f: &std::sync::Arc<crate::board::Frame>,
    fold: [bool; NCH],
) -> FastFrame {
    let window = m.time_per_div() as f64 * HDIV as f64;
    let x_per_sample = f.dt / m.time_per_div() as f64 * w as f64 / HDIV as f64;
    let proj = projection(m, w, h, x_per_sample);
    if x_per_sample > 0.5 || f.envelope || f.roll || f.scan {
        // Preserve the 3600-point sparse record and its trigger/interpolation policy until that source migrates.
        let rec = crate::board::to_record(f, m, 3600, window, m.delay as f64 - window / 2.0);
        let fold = std::array::from_fn(|c| {
            (fold[c] && m.ch[c].on).then(|| crate::fastdraw::Fold::of(rec.ch[c].as_slice(), 0..rec.ch[c].len()))
        });
        let triggered = rec.triggered;
        return FastFrame { plan: FastPlan { route: FastRoute::Rec(rec), fold, projection: proj }, triggered };
    }
    let meta = frame_timing(m, f);
    let Placed { projection, trace, fold } = kernel::place(
        std::array::from_fn(|c| SamplesRef::Volts(&f.ch[c])),
        &meta,
        &proj,
        crate::board::sw_trig_level(m, f),
        fold,
    );
    FastFrame {
        triggered: trace.outcome.triggered(&meta),
        plan: FastPlan { route: FastRoute::Raw(trace), projection, fold },
    }
}

/// Channel `c` of a placed frame into a band of its collected differences (`fastdraw::diff_span`): the columns
/// `c_lo ..` of a map `w` x `h`, `h + 1` rows of them.
pub(crate) fn add_frame_band(c: usize, band: &mut [u16], c_lo: usize, f: &crate::board::Frame, p: &FastPlan) {
    match &p.route {
        FastRoute::Rec(rec) => {
            let bw = band.len() / (p.projection.rows + 1);
            p.trace_record(c, rec, (c_lo as i32, (c_lo + bw) as i32), |cx, r0, r1, wt| {
                crate::fastdraw::diff_span(band, bw, cx as usize - c_lo, r0 as usize, r1 as usize, wt);
            });
        }
        FastRoute::Raw(trace) => {
            kernel::add_placed_band(c, band, c_lo, &SamplesRef::Volts(&f.ch[c]), &p.projection, trace);
        }
    }
}

/// Channel `c` of a placed frame into the rows `r_lo` .. `r_lo + band.len() / w` of that channel's map (`w` x `h`),
/// held in `band` (the whole map: `r_lo` 0): one frame after the other straight into the row-major map, the tests'
/// reference for the fast path's drawing.
#[cfg(test)]
pub(crate) fn fast_deposit(c: usize, band: &mut [u16], r_lo: usize, f: &crate::board::Frame, p: &FastPlan) {
    let (w, h) = (p.projection.cols, p.projection.rows);
    match &p.route {
        FastRoute::Rec(rec) => {
            let (b0, b1) = (r_lo as i32, (r_lo + band.len() / w.max(1)).min(h) as i32);
            p.trace_record(c, rec, (0, w as i32), |cx, r0, r1, wt| {
                for r in r0.max(b0)..=r1.min(b1 - 1) {
                    let idx = (r - b0) as usize * w + cx as usize;
                    band[idx] = band[idx].saturating_add(wt);
                }
            });
        }
        FastRoute::Raw(trace) => match &trace.spans[c] {
            Some(s) => crate::fastdraw::add_spans(band, w, r_lo, s, 24),
            None => {
                if let Some(map) = kernel::raw_map(&p.projection, c, trace.trigger_at) {
                    crate::fastdraw::deposit_dots_band(band, w, r_lo, h, &f.ch[c], &map, 24);
                }
            }
        },
    }
}
