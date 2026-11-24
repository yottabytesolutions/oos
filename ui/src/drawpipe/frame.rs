// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

/// One frame of the fast path into `ph`; false if its trigger edge was not found near the hardware trigger point.
/// Sparse frames (fewer than 2 samples per column) take the record path (`sparse::draw_with`, else `board::to_record`).
#[cfg(test)]
pub fn draw_frame(ph: &mut Phosphor, m: &Model, f: &Frame, interp: Interp, stats: Option<Stats>) -> bool {
    draw_frame_reusing(ph, m, f, interp, stats, None)
}

/// Use a freshly acquired record only when its captured conversion inputs still match.
pub(crate) fn draw_frame_reusing(
    ph: &mut Phosphor,
    m: &Model,
    f: &Frame,
    interp: Interp,
    stats: Option<Stats>,
    saved: Option<(&crate::wave::Record, reuse::EdgeInputs)>,
) -> bool {
    let (w, h) = (ph.w, ph.h);
    let spacing = f.dt / m.time_per_div() as f64 * w as f64 / HDIV as f64;
    let projection = crate::picture::deposit::standard::projection(m, w, h, spacing);
    let route = route::route(f, &projection, m.acq, false);
    if route.screen_raster(projection.dots, f) || crate::deepmeas::deep_pairs(f) || crate::deepmeas::deep_raw_frame(f) {
        return fallback(ph, m, f, interp, saved);
    }
    let placed = crate::picture::deposit::standard::batch_plan(m, (w, h), f, None, None);
    let Some((projection, trace)) = placed.plan.raw() else {
        unreachable!("dense placement returns column spans or dots");
    };
    if let Some(stats) = stats {
        stats.record(&trace.outcome);
    }
    if ph.is_wide() || projection.format == crate::picture::deposit::count::CountFormat::Wide {
        crate::picture::deposit::standard::wide_frame(ph, f, &placed.plan);
        return placed.triggered;
    }
    for c in 0..NCH {
        if let Some(spans) = &trace.spans[c] {
            ph.edit_channel(c, |map| {
                crate::picture::deposit::kernel::paint_spans(map, w, 0, spans, 24);
                let (start, end) = crate::fastdraw::span_rows(spans);
                start.min(end)..end
            });
        } else if let Some(map) = crate::picture::deposit::kernel::raw_map(projection, c, trace.trigger_at) {
            ph.edit_channel(c, |pixels| crate::fastdraw::deposit_dots(pixels, w, h, &f.ch[c], &map, 24));
        }
    }
    placed.triggered
}

fn fallback(
    ph: &mut Phosphor,
    m: &Model,
    f: &Frame,
    interp: Interp,
    saved: Option<(&crate::wave::Record, reuse::EdgeInputs)>,
) -> bool {
    let owned;
    let rec = if let Some((rec, _)) = saved.filter(|(rec, input)| input.matches(m, f, rec)) {
        rec
    } else {
        if let Some(t) = crate::sparse::draw_with(ph, m, f, 24, interp) {
            return t;
        }
        let window = m.time_per_div() as f64 * HDIV as f64;
        owned = crate::board::display_record(f, m, 3600, window, m.delay as f64 - window / 2.0);
        &owned
    };
    for c in 0..NCH {
        if m.ch[c].on && !rec.ch[c].is_empty() {
            ph.draw_ex(c, m, (m.time_per_div(), m.delay), rec.t0, rec.dt, &rec.ch[c], 24, None, rec.pairs);
        }
    }
    rec.triggered
}

impl Stats<'_> {
    pub(crate) fn record(self, outcome: &crate::picture::deposit::kernel::TrigOutcome) {
        use crate::picture::deposit::kernel::TrigOutcome;
        if let Some(index) = outcome.edge_index() {
            self.edges[index] += 1;
        }
        if let TrigOutcome::Moved(distance) = *outcome {
            *self.off = (self.off.0 + distance, self.off.1.max(distance));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::picture::deposit::kernel::TrigOutcome;

    #[test]
    fn trigger_stats_count_classes_and_only_accumulate_moved_distances() {
        let (mut edges, mut off) = ([0; 4], (0.0, 0.0));
        for (outcome, want_edges, want_off) in [
            (TrigOutcome::Hardware, [0, 0, 0, 0], (0.0, 0.0)),
            (TrigOutcome::AtPoint, [1, 0, 0, 0], (0.0, 0.0)),
            (TrigOutcome::WrongSlope, [1, 1, 0, 0], (0.0, 0.0)),
            (TrigOutcome::Moved(3.0), [1, 1, 1, 0], (3.0, 3.0)),
            (TrigOutcome::Moved(1.0), [1, 1, 2, 0], (4.0, 3.0)),
            (TrigOutcome::NotFound, [1, 1, 2, 1], (4.0, 3.0)),
            (TrigOutcome::Moved(5.0), [1, 1, 3, 1], (9.0, 5.0)),
        ] {
            Stats { edges: &mut edges, off: &mut off }.record(&outcome);
            assert_eq!(edges, want_edges);
            assert_eq!(off, want_off);
        }
    }
}
