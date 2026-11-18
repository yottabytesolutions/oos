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
    pub(crate) fn raw(&self) -> Option<(&Projection, &RawTrace)> {
        match &self.route {
            FastRoute::Raw(trace) => Some((&self.projection, trace)),
            FastRoute::Rec(_) => None,
        }
    }

    pub(crate) fn into_raw(self) -> Option<RawTrace> {
        match self.route {
            FastRoute::Raw(trace) => Some(trace),
            FastRoute::Rec(_) => None,
        }
    }

    pub(crate) fn fold(&self, c: usize) -> Option<crate::fastdraw::Fold> {
        self.fold[c]
    }

    pub(crate) fn outcome(&self) -> Option<&kernel::TrigOutcome> {
        self.raw().map(|(_, trace)| &trace.outcome)
    }

    pub(crate) fn rows(&self, c: usize) -> (usize, usize) {
        match &self.route {
            FastRoute::Raw(p) => p.spans[c].as_ref().map_or((0, self.projection.rows), crate::fastdraw::span_rows),
            FastRoute::Rec(rec) => self.projection.record_rows(c, &rec.ch[c]),
        }
    }

    pub(crate) fn hit_bound(&self) -> u64 {
        let proj = &self.projection;
        match &self.route {
            FastRoute::Raw(trace)
                if proj.dots && (0..NCH).any(|c| proj.ch[c].is_some() && trace.spans[c].is_none()) =>
            {
                // The rounded sample-index edges can include an extra sample.
                ((1.0 / proj.x_per_sample).ceil().max(1.0) as u64).saturating_add(1)
            }
            FastRoute::Raw(_) => 1,
            FastRoute::Rec(rec) => {
                let spacing = rec.dt * proj.cols as f64 / (proj.time_per_div * HDIV as f64);
                // The legacy record rasterizer can include the segments on both column edges.
                ((1.0 / spacing).ceil().max(1.0) as u64).saturating_add(2)
            }
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

impl FastFrame {
    pub(crate) fn record(
        rec: crate::wave::Record,
        projection: Projection,
        fold: [Option<crate::fastdraw::Fold>; NCH],
    ) -> Self {
        Self { triggered: rec.triggered, plan: FastPlan { route: FastRoute::Rec(rec), fold, projection } }
    }

    fn placed(Placed { projection, trace, fold }: Placed, meta: &FrameTiming) -> Self {
        Self {
            triggered: trace.outcome.triggered(meta),
            plan: FastPlan { route: FastRoute::Raw(trace), projection, fold },
        }
    }
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
        return FastFrame::record(rec, proj, fold);
    }
    let meta = frame_timing(m, f);
    let placed = kernel::place(
        std::array::from_fn(|c| SamplesRef::Volts(&f.ch[c])),
        &meta,
        &proj,
        crate::board::sw_trig_level(m, f),
        fold,
    );
    FastFrame::placed(placed, &meta)
}

/// Channel `c` of a placed frame into a band of its collected differences (`fastdraw::diff_span`): the columns
/// `c_lo ..` of a map `w` x `h`, `h + 1` rows of them.
pub(crate) fn add_frame_band(c: usize, band: &mut [u16], c_lo: usize, f: &crate::board::Frame, p: &FastPlan) {
    add_sample_band(c, band, c_lo, SamplesRef::Volts(&f.ch[c]), p);
}

pub(crate) fn add_sample_band(c: usize, band: &mut [u16], c_lo: usize, samples: SamplesRef<'_>, p: &FastPlan) {
    match &p.route {
        FastRoute::Rec(rec) => {
            let bw = band.len() / (p.projection.rows + 1);
            p.trace_record(c, rec, (c_lo as i32, (c_lo + bw) as i32), |cx, r0, r1, wt| {
                crate::fastdraw::diff_span(band, bw, cx as usize - c_lo, r0 as usize, r1 as usize, wt);
            });
        }
        FastRoute::Raw(trace) => {
            kernel::add_placed_band(c, band, c_lo, &samples, &p.projection, trace);
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

/// Frozen legacy search policy shared by the Batch pool and the newest-record adapter.
pub(crate) fn batch_plan(
    m: &Model,
    (w, h): (usize, usize),
    f: &crate::board::Frame,
    words: Option<(&[u8], crate::drawpipe::CodeLayout)>,
    previous: Option<RawTrace>,
) -> FastFrame {
    let spacing = f.dt / m.time_per_div() as f64 * w as f64 / HDIV as f64;
    let pairs = crate::compress::live(f) && spacing <= 0.5;
    let (bytes, codes) = words.map_or((&[][..], None), |(bytes, layout)| (bytes, Some(layout)));
    use crate::fastdraw::Samples;
    let samples = std::array::from_fn(|c| match codes {
        Some(layout) => kernel::SamplesRef::Codes(layout.channel(bytes, c)),
        None => kernel::SamplesRef::Volts(&f.ch[c]),
    });
    let meta = frame_timing(m, f);
    let proj = projection(m, w, h, spacing);
    let src = &samples[m.trig_src];
    let level = crate::board::sw_trig_level_of(m, src.len(), |i| src.at(i));
    let col =
        if pairs { crate::compress::smooth_len(spacing) } else { (1.0 / spacing).floor().clamp(1.0, 256.0) as usize };
    let t = meta.trigger;
    let (ti, outcome) = kernel::refine(&meta, col, || {
        let half = (proj.time_per_div * HDIV as f64 / 2.0 / f.dt) as usize;
        if near_only(m, f) {
            crate::trignear::edge_near(src, 0, src.len(), f.pre, level, t.hysteresis, t.rising, t.either, half, col)
        } else {
            crate::trignear::edge_smooth_near(src, f.pre, level, t.hysteresis, t.rising, t.either, half, col, |_| true)
        }
    });
    if !pairs {
        return FastFrame::placed(kernel::place_at(samples, &meta, &proj, [false; NCH], ti, outcome, previous), &meta);
    }
    let mut reused = previous.map_or_else(|| std::array::from_fn(|_| None), |trace| trace.spans);
    let spans = std::array::from_fn(|c| {
        kernel::raw_map(&proj, c, ti).map(|map| {
            let mut spans = reused[c].take().unwrap_or_default();
            crate::compress::spans_into(w, h, &f.ch[c], &map, proj.dots, &mut spans);
            spans
        })
    });
    let trace = RawTrace { trigger_at: ti, outcome, spans };
    FastFrame::placed(Placed { projection: proj, trace, fold: [None; NCH] }, &meta)
}

// AC and LF reject hide the comparator's signal from software. An exact edge trigger is already at `pre`.
fn near_only(m: &Model, f: &crate::board::Frame) -> bool {
    !matches!(m.trig_coupling(), TrigCoup::Dc | TrigCoup::HfReject) || (f.pre_exact && m.trig_type == TrigType::Edge)
}
