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
        refine_near(src, 0, src.len(), f.pre, near_only(m, f), level, hyst, rising, either, half, col).unwrap_or_else(
            || crate::fastdraw::edge_smooth(src, f.pre, level, hyst, rising, either, half, col, |_| true),
        )
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
    dense(ph, m, f, x_per_sample, pairs, present, stats, refine, deposit)
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
