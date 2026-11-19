// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::board::record::{raw_window, record_trigger, resample_record};
use std::sync::Arc;

type ChannelValues = [[f64; CHEAP.len()]; NCH];

pub(crate) struct TapFolds {
    generation: Option<u64>,
    screen_gate: Option<(f32, f32)>,
    latest: ChannelValues,
    displaced: ChannelValues,
}

impl TapFolds {
    pub(crate) fn compute(f: &Arc<Frame>, m: &Model, generation: Option<u64>, folds: &mut Folds) -> Option<Self> {
        folds.clear();
        folds.extend(empty_folds(m));
        let len = f.ch.iter().map(Vec::len).max().unwrap_or(0);
        if folds.is_empty() || len < 2 || crate::compress::live(f) {
            return None;
        }
        let n = crate::sparse::POINTS;
        let window = m.time_per_div() as f64 * HDIV as f64;
        let t_first = m.delay as f64 - window / 2.0;
        let (ti, triggered) = record_trigger(f, m, n, window, t_first);
        let (i0, i1, t0) = raw_window(ti, t_first, window, f.dt, len);
        let screen_pairs = window / n as f64 / f.dt >= 2.0 && m.acq != AcqMode::HiRes;
        let mut screen = None;
        let screen_record = || resample_record(f, m, n, window, t_first, ti, triggered);
        let mut latest = [[f64::NAN; CHEAP.len()]; NCH];
        for (c, values) in latest.iter_mut().enumerate() {
            if !folds.iter().any(|&((_, channel), _)| channel == c) {
                continue;
            }
            let (samples, t0, dt) = if m.acq != AcqMode::HiRes && !f.envelope && f.ch[c].len() >= 2 {
                (f.ch[c].as_slice(), t0 - i0 as f64 * f.dt, f.dt)
            } else {
                let record = screen.get_or_insert_with(screen_record);
                (record.ch[c].as_slice(), record.t0, record.dt)
            };
            let std =
                folds.iter().any(|&((kind, channel), _)| channel == c && matches!(kind, Meas::StdDev | Meas::VrmsAc));
            let gate = measurement_slice(m, samples, t0 - m.deskew_ch[c], dt);
            if let Some(values_in_gate) = gate.and_then(|(samples, ..)| Values::of(samples, std)) {
                let pairs = f.envelope || (screen_pairs && (i1 > f.ch[c].len() || i1 - i0 <= n));
                for (value, kind) in values.iter_mut().zip(CHEAP) {
                    if !pairs || !needs_samples(kind) {
                        *value = values_in_gate.get(kind);
                    }
                }
            }
        }
        let displaced = if !f.roll && !f.scan && fold_at(f, m, folds, window, t_first, ti) {
            let mut values = [[f64::NAN; CHEAP.len()]; NCH];
            for &((kind, channel), fold) in folds.iter() {
                if fold.n > 0 {
                    values[channel][kind_index(kind)] = fold.sum;
                }
            }
            values
        } else {
            displaced_screen(screen.get_or_insert_with(screen_record), folds)
        };
        let screen_gate = screen_gate(m);
        Some(Self { generation, screen_gate, latest, displaced })
    }

    pub(crate) fn current(&self, generation: u64) -> bool {
        self.generation == Some(generation)
    }

    pub(crate) fn matches_gate(&self, m: &Model) -> bool {
        self.screen_gate == screen_gate(m)
    }

    pub(crate) fn latest(&self, kind: Meas, channel: usize) -> Option<f64> {
        finite_value(&self.latest[channel], kind)
    }

    pub(crate) fn displaced(&self, kind: Meas, channel: usize) -> Option<f64> {
        finite_value(&self.displaced[channel], kind)
    }
}

fn finite_value(values: &[f64; CHEAP.len()], kind: Meas) -> Option<f64> {
    let value = values[CHEAP.iter().position(|&k| k == kind)?];
    value.is_finite().then_some(value)
}

fn kind_index(kind: Meas) -> usize {
    CHEAP.iter().position(|&k| k == kind).expect("folds_for only selects cheap measurements")
}

fn displaced_screen(record: &crate::wave::Record, folds: &Folds) -> ChannelValues {
    // Short records are measured on their interpolated points rather than on raw samples.
    let mut values = [[f64::NAN; CHEAP.len()]; NCH];
    let mut samples: [Option<crate::fastdraw::Fold>; NCH] = [None; NCH];
    for &((kind, channel), _) in folds {
        if record.pairs && needs_samples(kind) {
            continue;
        }
        let v = &record.ch[channel];
        let sample = samples[channel].get_or_insert_with(|| crate::fastdraw::Fold::of(v, 0..v.len()));
        if let Some(value) = sample.get(kind) {
            values[channel][kind_index(kind)] = value;
        }
    }
    values
}

pub(crate) fn measurement_view(m: &Model) -> (f32, f32) {
    if m.zoom {
        (tscales()[m.zscale], m.zdelay)
    } else {
        (m.time_per_div(), m.delay)
    }
}

fn screen_gate(m: &Model) -> Option<(f32, f32)> {
    (!m.meas_full_record).then(|| measurement_view(m))
}

pub(crate) fn measurement_bounds(m: &Model, len: usize, t0: f64, dt: f64) -> (usize, usize) {
    let Some((scale, delay)) = screen_gate(m) else {
        return (0, len);
    };
    let half = scale as f64 * HDIV as f64 * 0.5;
    let index = |time: f64| {
        let fractional = (time - t0) / dt;
        let integer = fractional.round();
        // Snap only the numerical fringe introduced by the f32 view.
        let fractional = if (fractional - integer).abs() <= 1e-5 { integer } else { fractional };
        (fractional.ceil().max(0.0) as usize).min(len)
    };
    // Half-open physical time: left <= sample_time < right.
    let lo = index(delay as f64 - half);
    let hi = index(delay as f64 + half).max(lo);
    (lo, hi)
}

pub(crate) fn measurement_slice<'a>(m: &Model, samples: &'a [f32], t0: f64, dt: f64) -> Option<(&'a [f32], f64, f64)> {
    if samples.len() < 2 || !dt.is_finite() || dt <= 0.0 {
        return None;
    }
    let (lo, hi) = measurement_bounds(m, samples.len(), t0, dt);
    (hi >= lo + 2).then_some((&samples[lo..hi], t0 + lo as f64 * dt, dt))
}
