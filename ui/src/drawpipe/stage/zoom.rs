// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::picture::PerMap;

#[cfg(test)]
mod tests;

impl View {
    pub(super) fn picture_sizes(&self) -> PerMap<(usize, usize)> {
        PerMap::new((self.w, self.h), self.zoom.map(route::ZoomView::size))
    }

    pub(in crate::drawpipe) fn zoom_hit_bound(&self, columns: usize) -> u64 {
        let window = f64::from(tscales()[self.m.zscale]) * f64::from(HDIV);
        let delay = zoom_delays(&self.m).fold(f64::from(self.m.zdelay).abs(), f64::max);
        standard::record_hit_bound(columns, crate::sparse::POINTS as f64, delay / window)
    }
}

fn zoom_delays(model: &Model) -> impl Iterator<Item = f64> + '_ {
    model.deskew_ch.iter().map(|&shift| (f64::from(model.zdelay) + shift).abs())
}

pub(super) fn place(batch: &Arc<Batch>, view: &Arc<View>, prepared: &mut Prepared, scratch: &mut Frame, bytes: &[u8]) {
    let zoom = prepared.zoom.get_or_insert_with(Default::default);
    let previous_detail = zoom.plan.take().and_then(|previous| previous.plan.recycle(&mut zoom.screen));
    let previous_main = prepared.plan.take().and_then(|previous| previous.plan.recycle(&mut prepared.screen));

    prepared.codes = None;
    prepared.decoded_dots = false;
    let newest = newest_at(batch, prepared.index);
    let decode = |frame: &mut Frame| {
        batch.decode.decode(prepared.index, bytes, frame);
        batch.header.tag(frame);
    };
    let coded = place_codes(
        batch,
        view,
        (prepared.index, bytes),
        scratch,
        (&mut prepared.screen, &mut zoom.screen),
        &mut prepared.refine,
        (previous_main, previous_detail),
    );
    if let Some((main, detail, layout, ti)) = coded {
        prepared.codes = layout.reduced_capture().then_some(layout);
        prepared.measured = newest.is_none()
            && !prepared.folds.is_empty()
            && fold_at(scratch, &view.m, (bytes, layout), ti, &mut prepared.folds, decode);
        prepared.plan = Some(main);
        zoom.plan = Some(detail);
        return;
    }
    Cause::ZoomDecoded.mark();
    let frame = decoded_newest_at(batch, prepared.index).unwrap_or_else(|| {
        decode(scratch);
        scratch
    });
    let (projections, _) = route::projection(view, frame.dt);
    let (main, detail) = projections.into_parts();
    let detail = detail.expect("the claimed view includes the detail projection");
    let (frame_plan, zoom_plan, ti) =
        standard::zoom_plans(&view.m, frame, (main, detail), (&mut prepared.screen, &mut zoom.screen));
    prepared.measured =
        newest.is_none() && crate::framestats::fold_frame_with(frame, &view.m, &mut prepared.folds, Some(ti));
    prepared.plan = Some(frame_plan);
    zoom.plan = Some(zoom_plan);
}

fn fold_at(
    scratch: &mut Frame,
    model: &Model,
    (bytes, layout): (&[u8], CodeLayout),
    ti: f64,
    folds: &mut crate::framestats::Folds,
    decode: impl FnOnce(&mut Frame),
) -> bool {
    if !crate::framestats::requires_frame(model, scratch, folds) {
        let timing = Frame { dt: scratch.dt, fixed_ti: Some(ti), ..Frame::default() };
        if let Some(measured) = crate::framestats::fold_codes(&timing, model, bytes, layout, folds) {
            return measured;
        }
    }
    Cause::ZoomMeasurementDecode.mark();
    decode(scratch);
    crate::framestats::fold_frame_with(scratch, model, folds, Some(ti))
}

fn place_codes(
    batch: &Arc<Batch>,
    view: &Arc<View>,
    (index, bytes): (usize, &[u8]),
    frame: &mut Frame,
    buffers: (&mut [Vec<f32>; NCH], &mut [Vec<f32>; NCH]),
    refine_time: &mut Duration,
    (previous_main, previous_detail): (Option<kernel::RawTrace>, Option<kernel::RawTrace>),
) -> Option<(standard::FastFrame, standard::FastFrame, CodeLayout, f64)> {
    let (_, layout, lanes) = feed::placement_words(batch, index, bytes, frame)?;
    batch.header.tag(frame);
    let (projections, _) = route::projection(view, frame.dt);
    let (main, detail) = projections.into_parts();
    let channels = std::array::from_fn(|c| layout.channel(bytes, c));
    let detail = detail?;
    let (main, detail, ti) = if layout.reduced_capture() {
        standard::zoom_reduced_plans(
            &view.m,
            frame,
            (main, detail),
            (channels, lanes),
            (previous_main, previous_detail),
            refine_time,
        )
    } else {
        standard::zoom_code_plans(
            &view.m,
            frame,
            (main, detail, layout.has_header()),
            channels,
            buffers,
            previous_detail,
            refine_time,
        )
    }?;
    Some((main, detail, layout, ti))
}
