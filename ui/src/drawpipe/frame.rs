// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

/// One frame of the fast path into `ph`; false if its trigger edge was not found near the hardware trigger point.
/// Sparse frames (fewer than 2 samples per column) take the record path (`sparse::draw_with`, else `board::to_record`).
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
    let tdiv = m.time_per_div() as f64;
    let x_per_sample = f.dt / tdiv * w as f64 / HDIV as f64;
    // min/max pairs of the live compression (compress.rs) take the dense path with a pair deposit
    let pairs = crate::compress::live(f) && x_per_sample <= 0.5;
    // the raw screen window of a deep record (`deepmeas`, once a second) takes the record path of its record's pair
    // frames: drawn densely it was 1.44 times their brightness (lines) and the grade swung (docs/fork/flicker.md)
    if !pairs && (x_per_sample > 0.5 || f.envelope || f.roll || f.scan || crate::deepmeas::deep_raw_frame(f)) {
        return fallback(ph, m, f, interp, saved);
    }
    let src = &f.ch[m.trig_src];
    let present: [bool; NCH] = std::array::from_fn(|c| !f.ch[c].is_empty());
    let refine = |hyst: f32, rising: bool, either: bool, half: usize, col: usize| {
        let level = crate::board::sw_trig_level(m, f);
        let trigger = crate::fastdraw::EdgeSearch { level, hysteresis: hyst, rising, either };
        refine_near(src, 0, src.len(), f.pre, near_only(m, f), level, hyst, rising, either, half, col)
            .unwrap_or_else(|| crate::fastdraw::edge_smooth(src.as_slice(), f.pre, trigger, half, col, |_| true))
    };
    let deposit = |c: usize, map: &mut [u16], xm: &crate::fastdraw::Map| {
        if pairs {
            crate::compress::deposit_pairs(map, w, h, &f.ch[c], xm, 24, m.dots_at(x_per_sample));
        } else if m.dots_at(x_per_sample) {
            crate::fastdraw::deposit_dots(map, w, h, &f.ch[c], xm, 24);
        } else {
            crate::fastdraw::deposit(map, w, h, &f.ch[c], xm, 24);
        }
    };
    draw_projected(ph, m, f, ProjectedInput { x_per_sample, pairs, present }, stats, refine, deposit)
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

// The Batch adapters keep their near-only search and whole-map deposit until F2.
pub(super) struct ProjectedInput {
    pub(super) x_per_sample: f64,
    pub(super) pairs: bool,
    pub(super) present: [bool; NCH],
}

pub(super) fn draw_projected(
    ph: &mut Phosphor,
    m: &Model,
    f: &Frame,
    input: ProjectedInput,
    stats: Option<Stats>,
    search: impl FnOnce(f32, bool, bool, usize, usize) -> (Option<f64>, Option<oos_dsp::edges::Edge>),
    mut deposit: impl FnMut(usize, &mut [u16], &crate::fastdraw::Map),
) -> bool {
    use crate::picture::deposit::{kernel, standard};
    let ProjectedInput { x_per_sample, pairs, present } = input;
    let proj = standard::projection(m, ph.w, ph.h, x_per_sample);
    let meta = standard::frame_timing(m, f);
    let t = meta.trigger;
    let col = if pairs {
        crate::compress::smooth_len(x_per_sample)
    } else {
        (1.0 / x_per_sample).floor().clamp(1.0, 256.0) as usize
    };
    let (ti, outcome) = kernel::refine(&meta, col, || {
        search(t.hysteresis, t.rising, t.either, (proj.time_per_div * HDIV as f64 / 2.0 / meta.dt) as usize, col)
    });
    if let Some(s) = stats {
        s.record(&outcome);
    }
    for (c, &present) in present.iter().enumerate() {
        if let Some(map) = kernel::raw_map(&proj, c, ti).filter(|_| present) {
            deposit(c, &mut ph.map[c], &map);
        }
    }
    outcome.triggered(&meta)
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
    fn projected_maps_preserve_supplied_spacing_for_samples_and_pairs() {
        let m = Model::default();
        let f = Frame { dt: 1e-8, pre: 500, ..Default::default() };
        let mut ph = Phosphor::new(97, 48);
        let spacing = 0.125;
        assert_ne!(spacing, f.dt / m.time_per_div() as f64 * ph.w as f64 / HDIV as f64);
        for pairs in [false, true] {
            let mut calls = 0;
            assert!(draw_projected(
                &mut ph,
                &m,
                &f,
                ProjectedInput { x_per_sample: spacing, pairs, present: [true, false, false, false] },
                None,
                |_, _, _, _, col| {
                    let want = if pairs { crate::compress::smooth_len(spacing) } else { 8 };
                    assert_eq!(col, want);
                    (Some(f.pre as f64), None)
                },
                |c, _, map| {
                    assert_eq!(c, 0);
                    assert_eq!(map.x_per_sample, spacing);
                    calls += 1;
                },
            ));
            assert_eq!(calls, 1);
        }
    }

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
