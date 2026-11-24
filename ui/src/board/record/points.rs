// SPDX-License-Identifier: GPL-2.0-or-later

use super::{Frame, NCH};

/// Sample the trigger-aligned grid into caller-owned buffers, including at dense input spacing.
pub(crate) fn point_channels(
    frame: &Frame,
    (n, window, first): (usize, f64, f64),
    ti: f64,
    channels: &mut [Vec<f32>; NCH],
) {
    let dt = window / n.max(1) as f64;
    for (values, out) in frame.ch.iter().zip(channels) {
        channel(values, (frame.dt, false), (n, dt, first), ti, out);
    }
}

pub(super) fn channel(
    values: &[f32],
    (sample_dt, envelope): (f64, bool),
    (n, dt, first): (usize, f64, f64),
    ti: f64,
    out: &mut Vec<f32>,
) {
    out.clear();
    if values.len() < 2 {
        return;
    }
    if !envelope && sample_dt > 2.0 * dt {
        oos_dsp::interp::lanczos8_grid(values, ti + first / sample_dt, dt / sample_dt, n, out);
    } else {
        out.extend((0..n).map(|k| {
            let x = ti + (first + k as f64 * dt) / sample_dt;
            oos_dsp::interp::linear(values, x).unwrap_or(f32::NAN)
        }));
    }
}
