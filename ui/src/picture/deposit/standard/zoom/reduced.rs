// SPDX-License-Identifier: GPL-2.0-or-later

use super::{codes::detail, *};
use crate::fastdraw::Codes;
use crate::picture::deposit::envelope::Layout;

pub(crate) fn zoom_reduced_plans(
    model: &Model,
    frame: &Frame,
    (main, zoom): (Projection, Projection),
    (codes, layout): ([Codes<'_>; NCH], Layout),
    (main_trace, zoom_trace): (Option<RawTrace>, Option<RawTrace>),
    refine_time: &mut Duration,
) -> Option<(FastFrame, FastFrame, f64)> {
    let shifts = model.deskew_ch;
    let shifted = model.ch.iter().zip(shifts).any(|(ch, shift)| ch.on && shift != 0.0);
    if !main.code_geometry() || !zoom.code_geometry() || (!frame.envelope && main.dots && shifted) {
        Cause::ZoomReducedGeometry.mark();
        return None;
    }
    let meta = frame_timing(model, frame);
    let route = route::route(frame, &main, model.acq, true);
    let source = SamplesRef::Codes(codes[model.trig_src]);
    let (ti, outcome) = batch_trigger(model, frame, &main, &meta, (&source, route), Some(refine_time));
    let place = |projection, trace| {
        place_map(projection, (&meta, ti, outcome), (&codes, layout), (shifts, shifted), frame.envelope, trace)
    };
    Some((place(main, main_trace), place(zoom, zoom_trace), ti))
}

fn place_map(
    projection: Projection,
    (meta, ti, outcome): (&FrameTiming, f64, kernel::TrigOutcome),
    (codes, layout): (&[Codes<'_>; NCH], Layout),
    (shifts, shifted): ([f64; NCH], bool),
    pairs: bool,
    previous: Option<RawTrace>,
) -> FastFrame {
    if !pairs && !projection.dots && (projection.x_per_sample > 0.5 || shifted) {
        return crate::fastdraw::with_scratch(|scratch| {
            detail::place(shifts, projection, (ti, outcome), meta, codes, &mut scratch.routing.zoom.filter, previous)
        });
    }
    let samples = codes.map(SamplesRef::Codes);
    let mut placed = if pairs {
        let mut spans = previous.map_or_else(Default::default, |trace| trace.spans);
        crate::fastdraw::with_scratch(|scratch| {
            if shifted {
                for (c, (shift, span)) in shifts.into_iter().zip(&mut spans).enumerate() {
                    let mut lane = std::array::from_fn(|_| None);
                    lane[c] = span.take();
                    let projection = Projection {
                        ch: std::array::from_fn(|i| (i == c).then_some(projection.ch[i]).flatten()),
                        delay: projection.delay + shift,
                        ..projection
                    };
                    scratch.envelope.pairs((&samples, Some(layout)), &projection, ti, &mut lane);
                    *span = lane[c].take();
                }
            } else {
                scratch.envelope.pairs((&samples, Some(layout)), &projection, ti, &mut spans);
            }
        });
        let trace = RawTrace { trigger_at: ti, outcome, spans };
        Placed { projection, route: Route::Zoom, trace, fold: [None; NCH] }
    } else {
        let route = if projection.dots { Route::Dots } else { Route::Dense };
        kernel::place_at(samples, &projection, (route, Some(layout)), (ti, outcome), previous)
    };
    placed.route = Route::Zoom;
    FastFrame::placed(placed, meta)
}
