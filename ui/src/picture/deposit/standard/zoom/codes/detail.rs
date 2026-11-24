// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::fastdraw::zoom::{Filter, Window};

pub(in crate::picture::deposit::standard::zoom) fn place(
    shifts: [f64; NCH],
    projection: Projection,
    (trigger_at, outcome): (f64, kernel::TrigOutcome),
    meta: &FrameTiming,
    codes: &[Codes<'_>; NCH],
    filter: &mut Filter,
    previous: Option<RawTrace>,
) -> FastFrame {
    let mut spans = previous.map_or_else(Default::default, |trace| trace.spans);
    let width = projection.cols as f64;
    let (span, _) = window(projection);
    for (c, ((codes, vertical), out)) in codes.iter().zip(projection.ch).zip(&mut spans).enumerate() {
        let Some(vertical) = vertical else {
            if let Some(span) = out {
                span.rows.clear();
            }
            continue;
        };
        let delay = f64::from((projection.delay + shifts[c]) as f32);
        let relative = ((delay - shifts[c] - projection.delay) / span) * width;
        let window = Window {
            start: trigger_at + (delay - span * 0.5) / meta.dt,
            step: (span / POINTS as f64) / meta.dt,
            column_origin: relative,
        };
        fixture_stage!(
            Resample,
            c,
            filter.spans(
                codes,
                window,
                (projection.cols, projection.rows),
                vertical,
                out.get_or_insert_with(Default::default)
            )
        );
    }
    FastFrame::placed(
        kernel::Placed {
            projection: Projection { dots: false, format: CountFormat::Narrow, ..projection },
            route: Route::Zoom,
            trace: RawTrace { trigger_at, outcome, spans },
            fold: [None; NCH],
        },
        meta,
    )
}
