// SPDX-License-Identifier: GPL-2.0-or-later

//! Adapters from the existing Standard and Batch routes to frozen kernel inputs.

use super::count::{Count, CountFormat};
use super::kernel::{self, FrameTiming, Placed, Projection, RawTrace, SamplesRef, TrigSnapshot, Vertical};
use crate::drawpipe::metrics::fallback::{self, Cause};
use crate::drawpipe::route::{self, hardware_position, Route};
use crate::drawpipe::stage::fixture_stage;
use crate::fastdraw::Samples;
use crate::model::*;
use std::time::Duration;

mod screen;
mod trigger;
pub(crate) use screen::trigger as screen_trigger;
pub(crate) use screen::AverageGrid;
pub(crate) use trigger::batch_trigger;
use trigger::timed_trigger;
pub(crate) use zoom::{zoom_code_plans, zoom_plans, zoom_reduced_plans, ZoomScratch};
mod zoom;

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
        format: CountFormat::for_hits(if m.dots_at(x_per_sample) && x_per_sample > 0.0 && x_per_sample <= 0.5 {
            dot_hit_bound(x_per_sample * w.min(crate::ui::batch_min_columns()).max(1) as f64 / w.max(1) as f64)
        } else {
            1
        }),
    }
}

pub(crate) fn frame_timing(m: &Model, f: &crate::board::Frame) -> FrameTiming {
    FrameTiming {
        dt: f.dt,
        pre: f.pre,
        anchor: f.trigger_anchor.point(f.pre),
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

pub(crate) fn dot_hit_bound(spacing: f64) -> u64 {
    // The rounded sample-index edges can include an extra sample.
    ((1.0 / spacing).ceil().max(1.0) as u64).saturating_add(1)
}

pub(crate) fn record_hit_bound(cols: usize, points: f64, delay_in_windows: f64) -> u64 {
    let columns = cols as f64;
    // trace_columns computes x in f64 before rounding to f32 at the column edges.
    let rounding = f32::EPSILON as f64 + (1.0 + delay_in_windows.abs()) * 8.0 * f64::EPSILON;
    let vertices = ((1.0 + (columns + 1.0) * rounding) * points / columns).ceil().max(1.0) as u64;
    // A column with n vertices intersects at most n + 1 adjacent segments.
    vertices.saturating_add(1)
}

pub(crate) struct FastPlan {
    route: FastRoute,
    projection: Projection,
    fold: [Option<crate::fastdraw::Fold>; NCH],
    origins: Option<[f64; NCH]>,
}

impl FastPlan {
    pub(crate) fn raw(&self) -> Option<(&Projection, &RawTrace)> {
        match &self.route {
            FastRoute::Raw(trace) => Some((&self.projection, trace)),
            FastRoute::Rec(_) => None,
        }
    }

    #[cfg(test)]
    pub(crate) fn into_raw(self) -> Option<RawTrace> {
        self.recycle(&mut Default::default())
    }

    pub(crate) fn recycle(self, screen: &mut [Vec<f32>; NCH]) -> Option<RawTrace> {
        match self.route {
            FastRoute::Raw(trace) => Some(trace),
            FastRoute::Rec(rec) => {
                *screen = rec.ch;
                None
            }
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

    pub(crate) fn weight(&self) -> f64 {
        if self.projection.dots {
            let spacing = match &self.route {
                FastRoute::Rec(rec) => {
                    rec.dt / self.projection.time_per_div * self.projection.cols as f64 / HDIV as f64
                }
                FastRoute::Raw(_) => self.projection.x_per_sample,
            };
            if spacing.is_finite() && spacing > 0.0 {
                return 24.0 / spacing;
            }
        }
        24.0
    }

    pub(crate) fn format(&self) -> CountFormat {
        self.projection.format
    }

    pub(crate) fn hit_bound(&self) -> u64 {
        let proj = &self.projection;
        match &self.route {
            FastRoute::Raw(trace)
                if proj.dots && (0..NCH).any(|c| proj.ch[c].is_some() && trace.spans[c].is_none()) =>
            {
                dot_hit_bound(proj.x_per_sample)
            }
            FastRoute::Raw(_) => 1,
            FastRoute::Rec(rec) if rec.dt == 0.0 && rec.ch.iter().all(Vec::is_empty) => 1,
            FastRoute::Rec(rec) => {
                let window = proj.time_per_div * HDIV as f64;
                let delay_in_windows = self.origins.map_or(proj.delay.abs(), |origins| {
                    origins.into_iter().fold(proj.delay.abs(), |delay, t0| delay.max((t0 + window / 2.0).abs()))
                }) / window;
                record_hit_bound(proj.cols, window / rec.dt, delay_in_windows)
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
            self.origins.map_or(rec.t0, |origins| origins[c]),
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
    pub(crate) route: Route,
    pub(crate) plan: FastPlan,
    pub(crate) triggered: bool,
}

impl FastFrame {
    pub(crate) fn record(
        rec: crate::wave::Record,
        projection: Projection,
        fold: [Option<crate::fastdraw::Fold>; NCH],
    ) -> Self {
        Self {
            triggered: rec.triggered,
            route: Route::Volts,
            plan: FastPlan { route: FastRoute::Rec(rec), fold, projection, origins: None },
        }
    }

    pub(crate) fn placed(Placed { projection, route, trace, fold }: Placed, meta: &FrameTiming) -> Self {
        Self {
            triggered: trace.outcome.triggered(meta),
            route,
            plan: FastPlan { route: FastRoute::Raw(trace), projection, fold, origins: None },
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
    fallback::direct(|| {
        let window = m.time_per_div() as f64 * HDIV as f64;
        let x_per_sample = f.dt / m.time_per_div() as f64 * w as f64 / HDIV as f64;
        let proj = projection(m, w, h, x_per_sample);
        let route = route::route(f, &proj, m.acq, false);
        Cause::DecodedDots.when(route == Route::Dots);
        if route == Route::Boxcar && !proj.dots {
            return batch_plan_timed(m, f, BatchPlacement { projection: proj, route, words: None }, None, None);
        }
        if x_per_sample > 0.5 || f.envelope || f.roll || f.scan || m.acq == AcqMode::HiRes {
            // Preserve the 3600-point sparse record and its trigger/interpolation policy until that source migrates.
            Cause::DirectRecord.mark();
            let first = m.delay as f64 - window / 2.0;
            use crate::board::record::{record_trigger, HW_POINTS};
            let (ti, triggered) = record_trigger(f, m, HW_POINTS, window, first);
            trigger::count_short_window(f, (first, first + window), ti, f.ch[m.trig_src].len());
            let rec = crate::board::resample_record(f, m, HW_POINTS, window, first, ti, triggered);
            let fold = std::array::from_fn(|c| {
                (fold[c] && m.ch[c].on).then(|| crate::fastdraw::Fold::of(rec.ch[c].as_slice(), 0..rec.ch[c].len()))
            });
            return FastFrame::record(rec, proj, fold);
        }
        let meta = frame_timing(m, f);
        if hardware_position(m, f).is_some() {
            return batch_plan_timed(m, f, BatchPlacement { projection: proj, route, words: None }, None, None);
        }
        let placed = kernel::place(
            std::array::from_fn(|c| SamplesRef::Volts(&f.ch[c])),
            &meta,
            &proj,
            crate::board::sw_trig_level(m, f),
            fold,
        );
        FastFrame::placed(placed, &meta)
    })
}

/// Channel `c` of a placed frame into a band of its collected differences (`fastdraw::diff_span`): the columns
/// `c_lo ..` of a map `w` x `h`, `h + 1` rows of them.
pub(crate) fn add_frame_band(c: usize, band: &mut [u16], c_lo: usize, f: &crate::board::Frame, p: &FastPlan) {
    add_sample_band(c, band, c_lo, SamplesRef::Volts(&f.ch[c]), p);
}

pub(crate) fn add_sample_band<T: Count>(
    c: usize,
    band: &mut [T],
    c_lo: usize,
    samples: SamplesRef<'_>,
    p: &FastPlan,
) -> std::ops::Range<usize> {
    match &p.route {
        FastRoute::Rec(rec) => {
            let bw = band.len() / (p.projection.rows + 1);
            p.trace_record(c, rec, (c_lo as i32, (c_lo + bw) as i32), |cx, r0, r1, wt| {
                crate::fastdraw::diff_span(band, bw, cx as usize - c_lo, r0 as usize, r1 as usize, wt);
            });
            let (start, end) = p.rows(c);
            start.min(end)..end
        }
        FastRoute::Raw(trace) => kernel::add_placed_band(c, band, c_lo, &samples, &p.projection, trace),
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
    let projection = projection(m, w, h, spacing);
    let words = words.and_then(|(bytes, layout)| Some((bytes, layout, super::envelope::Layout::new(&layout)?)));
    let route = route::route(f, &projection, m.acq, words.is_some());
    batch_plan_timed(m, f, BatchPlacement { projection, route, words }, previous, None)
}

pub(crate) struct BatchPlacement<'a> {
    pub(crate) projection: Projection,
    pub(crate) route: Route,
    pub(crate) words: Option<(&'a [u8], crate::drawpipe::CodeLayout, super::envelope::Layout)>,
}

pub(crate) fn batch_plan_timed(
    m: &Model,
    f: &crate::board::Frame,
    BatchPlacement { projection: proj, route, words }: BatchPlacement<'_>,
    previous: Option<RawTrace>,
    refine_time: Option<&mut Duration>,
) -> FastFrame {
    let samples = std::array::from_fn(|c| match words {
        Some((bytes, layout, _)) => kernel::SamplesRef::Codes(layout.channel(bytes, c)),
        None => kernel::SamplesRef::Volts(&f.ch[c]),
    });
    let meta = frame_timing(m, f);
    let (ti, outcome) = if route == Route::Boxcar {
        let (ti, outcome) = screen::trigger(m, f, &proj, None, refine_time);
        (ti, if outcome == kernel::TrigOutcome::NotFound { outcome } else { kernel::TrigOutcome::Hardware })
    } else {
        batch_trigger(m, f, &proj, &meta, (&samples[m.trig_src], route), refine_time)
    };
    let half = proj.time_per_div * HDIV as f64 / 2.0;
    trigger::count_short_window(f, (proj.delay - half, proj.delay + half), ti, samples[m.trig_src].len());
    let layout = words.map(|(_, _, layout)| layout);
    let placed = match route {
        Route::Pairs | Route::Boxcar => {
            let mut spans = previous.map_or_else(Default::default, |trace| trace.spans);
            crate::fastdraw::with_scratch(|scratch| match route {
                Route::Pairs => scratch.envelope.pairs((&samples, layout), &proj, ti, &mut spans),
                _ => scratch.envelope.boxcar(&samples, &proj, (ti, f.dt), &mut spans),
            });
            let trace = RawTrace { trigger_at: ti, outcome, spans };
            Placed { projection: proj, route, trace, fold: [None; NCH] }
        }
        _ => kernel::place_at(samples, &proj, (route, layout), (ti, outcome), previous),
    };
    FastFrame::placed(placed, &meta)
}

pub(crate) fn screen_plan(
    m: &Model,
    f: &crate::board::Frame,
    placement: BatchPlacement<'_>,
    interp: crate::sparse::Interp,
    samples: &mut [Vec<f32>; NCH],
    previous: Option<RawTrace>,
) -> FastFrame {
    screen::place(m, f, placement, interp, samples, previous)
}

/// Exceptional direct-frame consumer shares the same differences and prefix kernels as Batch.
pub(crate) fn wide_frame(ph: &mut crate::wave::Phosphor, frame: &crate::board::Frame, plan: &FastPlan) {
    ph.set_format(CountFormat::Wide);
    for mut band in super::DiffBand::bands((ph.w, ph.h), plan.projection.ch.map(|ch| ch.is_some())) {
        band.set_format(CountFormat::Wide);
        band.paint(SamplesRef::Volts(&frame.ch[band.channel]), plan);
        band.fold_picture([None, None], ph);
    }
}
