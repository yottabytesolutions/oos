// SPDX-License-Identifier: GPL-2.0-or-later

//! Screen-grid placement for interpolated and retained records.

use super::*;
use crate::board::{record, Frame};
use crate::sparse::{self, Interp, POINTS};

mod polyphase;

pub(crate) fn trigger(
    m: &Model,
    f: &Frame,
    projection: &Projection,
    mut wide: Option<&mut crate::trignear::WideRefine<'_>>,
    refine_time: Option<&mut Duration>,
) -> (f64, kernel::TrigOutcome) {
    use kernel::TrigOutcome;
    if let Some(ti) = hardware_position(m, f) {
        #[cfg(test)]
        crate::trignear::fixture::placed(f, ti);
        return (ti, TrigOutcome::Hardware);
    }
    #[cfg(test)]
    if !f.roll && !f.scan && crate::trignear::fixture::bypass() {
        return (f.pre as f64, TrigOutcome::Hardware);
    }
    Cause::TriggerFromVolts.when(!f.roll && !f.scan && !m.trig_on_la() && !m.bus_on_la());
    let window = projection.time_per_div * HDIV as f64;
    let (ti, found) = timed_trigger(refine_time, || {
        record::record_trigger_with(f, m, (POINTS, window, projection.delay - window / 2.0), wide.as_deref_mut())
    });
    #[cfg(test)]
    crate::trignear::fixture::placed(f, ti);
    let outcome = match found {
        false => TrigOutcome::NotFound,
        true if ti == f.pre as f64 => TrigOutcome::AtPoint,
        true => TrigOutcome::Moved((ti - f.pre as f64).abs()),
    };
    (ti, outcome)
}

pub(super) fn place(
    m: &Model,
    f: &Frame,
    placement: BatchPlacement<'_>,
    interp: Interp,
    samples: &mut [Vec<f32>; NCH],
    previous: Option<RawTrace>,
) -> FastFrame {
    let BatchPlacement { projection, route, words } = placement;
    let window = projection.time_per_div * HDIV as f64;
    let first = projection.delay - window / 2.0;
    let dt = window / POINTS as f64;
    if let Some((bytes, layout, _)) = words {
        let (ti, outcome) = trigger(m, f, &projection, None, None);
        let mut spans = previous.map_or_else(Default::default, |trace| trace.spans);
        polyphase::place(&projection, (first, dt, f.dt, ti), |c| layout.channel(bytes, c), &mut spans);
        let trace = RawTrace { trigger_at: ti, outcome, spans };
        let mut placed = FastFrame::placed(Placed { projection, route, trace, fold: [None; NCH] }, &frame_timing(m, f));
        placed.triggered = outcome != kernel::TrigOutcome::NotFound;
        return placed;
    }
    for points in &mut *samples {
        points.clear();
    }
    // The pool uses the exact screen points. The legacy interpolation switch still applies on direct redraws.
    let interp = if route == Route::Upsampled { Interp::Exact } else { interp };
    let sparse = matches!(route, Route::Upsampled | Route::Record)
        && f.fixed_ti.is_none()
        && m.acq != AcqMode::HiRes
        && !f.envelope
        && dt / f.dt < 2.0
        && project_decoded(m, f, interp, samples).is_some();
    let (triggered, pairs) = if sparse {
        Cause::DecodedSparse.mark();
        (true, false)
    } else {
        Cause::DecodedScreen.mark();
        let (ti, outcome) = trigger(m, f, &projection, None, None);
        let pairs = record::resample_channels(f, m, (POINTS, window, first), ti, samples);
        (outcome != kernel::TrigOutcome::NotFound, pairs)
    };
    let rec = crate::wave::Record {
        t0: first,
        dt,
        ch: std::mem::take(samples),
        triggered,
        partial: f.partial,
        raw: None,
        la: None,
        pairs,
    };
    let mut plan = FastFrame::record(rec, projection, [None; NCH]);
    plan.route = route;
    plan
}

fn project_decoded(m: &Model, f: &Frame, interp: Interp, samples: &mut [Vec<f32>; NCH]) -> Option<bool> {
    let mut project = || {
        Cause::TriggerFromVolts.when(
            m.trig_type == TrigType::Edge
                && m.trig_line().is_none()
                && !f.roll
                && !f.scan
                && f.ch[m.trig_src].len() > 1,
        );
        sparse::project(m, f, interp, |c, _, _, values| samples[c].extend_from_slice(values))
    };
    #[cfg(test)]
    return crate::trignear::fixture::sparse(f, &mut project);
    #[cfg(not(test))]
    project()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drawpipe::CodeLayout;

    #[test]
    fn code_screen_retains_the_trigger_outcome_and_legacy_trigger_credit() {
        use kernel::TrigOutcome;
        let layout = CodeLayout::uncorrected(1, [Some(0), None, None, None], [(1.0, 0.0); NCH], |_, _| 0.0);
        let words = [0; 256];
        let model = Model::default();
        let dt = f64::from(model.time_per_div()) * f64::from(HDIV) / 64.0;
        let projection = projection(&model, 32, 16, 0.75);
        for (fixed_ti, expected) in [(None, TrigOutcome::NotFound), (Some(64.0), TrigOutcome::Hardware)] {
            let frame = Frame {
                pre: 64,
                dt,
                fixed_ti,
                ch: [vec![0.0; 128], Vec::new(), Vec::new(), Vec::new()],
                ..Frame::default()
            };
            let placement = BatchPlacement {
                projection,
                route: Route::Upsampled,
                words: Some((&words, layout, crate::picture::deposit::envelope::Layout::new(&layout).unwrap())),
            };
            let plan = place(&model, &frame, placement, Interp::Exact, &mut Default::default(), None);
            assert_eq!(plan.plan.outcome(), Some(&expected));
            assert_eq!(plan.triggered, fixed_ti.is_some());
        }
    }
}
