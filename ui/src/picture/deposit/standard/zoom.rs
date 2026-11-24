// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::board::{record, Frame};
use crate::sparse::POINTS;

mod codes;
mod reduced;
pub(crate) use codes::{zoom_code_plans, ZoomScratch};
pub(crate) use reduced::zoom_reduced_plans;

#[derive(Clone, Copy)]
struct Alignment {
    trigger: f64,
    raw_trigger: f64,
}

/// Render both windows from the original record's accepted trigger and screen sampling rule.
pub(crate) fn zoom_plans(
    m: &Model,
    frame: &Frame,
    (main, detail): (Projection, Projection),
    (main_samples, detail_samples): (&mut [Vec<f32>; NCH], &mut [Vec<f32>; NCH]),
) -> (FastFrame, FastFrame, f64) {
    let (ti, outcome) = screen::trigger(m, frame, &main, None, None);
    let triggered = outcome != kernel::TrigOutcome::NotFound;
    let len = frame.ch.iter().map(Vec::len).max().unwrap_or(0);
    let (overview, detail_alignment) = align(main, ti, frame.dt, len);
    let resample = |(n, dt), windows: [(f64, f64); NCH], channels: &mut [Vec<f32>; NCH]| {
        for ((values, out), (first, ti)) in frame.ch.iter().zip(channels).zip(windows) {
            record::resample_channel(values, (frame.dt, frame.envelope), m.acq, (n, dt, first), ti, out);
        }
    };
    (
        captured(m, frame, main, overview, triggered, std::mem::take(main_samples), resample),
        captured(m, frame, detail, detail_alignment, triggered, std::mem::take(detail_samples), resample),
        ti,
    )
}

fn align(main: Projection, ti: f64, dt: f64, len: usize) -> (Alignment, Alignment) {
    let (span, first) = window(main);
    let (i0, _, t0) = record::raw_window(ti, first, span, dt, len);
    // Match the trigger reconstructed by captured_record_view from Raw::t0 and the window start index.
    let raw_trigger = -(t0 - i0 as f64 * dt) / dt;
    (Alignment { trigger: ti, raw_trigger }, Alignment { trigger: raw_trigger, raw_trigger })
}

fn window(projection: Projection) -> (f64, f64) {
    let seconds = projection.time_per_div * f64::from(HDIV);
    (seconds, projection.delay - seconds / 2.0)
}

fn captured(
    model: &Model,
    frame: &Frame,
    mut projection: Projection,
    alignment: Alignment,
    triggered: bool,
    mut channels: [Vec<f32>; NCH],
    resample: impl FnOnce((usize, f64), [(f64, f64); NCH], &mut [Vec<f32>; NCH]),
) -> FastFrame {
    let (span, first) = window(projection);
    let dt = span / POINTS as f64;
    projection.x_per_sample = dt / projection.time_per_div * (projection.cols as f64 / f64::from(HDIV));
    projection.dots = model.dots_at(projection.x_per_sample);
    projection.format = CountFormat::Narrow;
    let pairs = dt / frame.dt >= 2.0 && model.acq != AcqMode::HiRes;
    let windows = std::array::from_fn(|c| {
        let shift = model.deskew_ch[c];
        if shift != 0.0 {
            let shifted_delay = (projection.delay + shift) as f32;
            (f64::from(shifted_delay) - span * 0.5, alignment.raw_trigger)
        } else {
            (first, alignment.trigger)
        }
    });
    let origins = std::array::from_fn(|c| windows[c].0 - model.deskew_ch[c]);
    resample((POINTS, dt), windows, &mut channels);
    let rec = crate::wave::Record {
        t0: first,
        dt,
        ch: channels,
        triggered,
        partial: frame.partial,
        raw: None,
        la: None,
        pairs,
    };
    let mut plan = FastFrame::record(rec, projection, [None; NCH]);
    plan.route = Route::Zoom;
    plan.plan.origins = Some(origins);
    plan
}

impl FastPlan {
    pub(crate) fn paint_zoom_lines<T: Count>(
        &self,
        band: &mut [T],
        first: usize,
        channel: usize,
    ) -> Option<std::ops::Range<usize>> {
        use crate::fastdraw::zoom::{self, Geometry};

        let FastRoute::Rec(rec) = &self.route else { return None };
        let projection = &self.projection;
        let origins = self.origins?;
        if projection.dots {
            return None;
        }
        let ch = projection.ch[channel]?;
        let samples = &rec.ch[channel];
        let geometry = Geometry::new(
            projection.cols,
            (rec.dt, projection.time_per_div, projection.delay, origins[channel]),
            samples.len(),
        );
        zoom::paint(samples, geometry, (band, projection.rows, first), (ch.volts_per_div, ch.offset), channel)
    }
}
