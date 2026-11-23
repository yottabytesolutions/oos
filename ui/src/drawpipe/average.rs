// SPDX-License-Identifier: GPL-2.0-or-later

//! Ordered affine folds on the decoded screen grid.
//!
//! For a slice of n finite inputs, gain is r^n and bias is alpha times the
//! weighted sample sum. A nonfinite sample or an absent channel breaks that
//! point's dependency on the preceding slices. The UI supplies the seed at
//! FIFO intake, where the previous displayed record and Clear/RUN are known.

use crate::{board::Frame, model::NCH, wave::Record};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

use super::{clock, Counters, Lost};
use oos_core::change::Identity;
use std::time::Instant;
#[cfg(any(target_os = "linux", test))]
const RESERVED_SAMPLES: usize =
    crate::board::record::HW_POINTS * NCH * (size_of::<Point>() / size_of::<f32>()) + METADATA_SAMPLES;
const METADATA_SAMPLES: usize = size_of::<BatchResult>().div_ceil(size_of::<f32>());

#[derive(Clone, Copy)]
struct Point {
    bias: f32,
    // A finite first sample means this point still depends on its predecessor.
    first: f32,
}

impl Point {
    fn seed(self, gain: f32) -> f32 {
        if self.first.is_finite() {
            gain.mul_add(self.first, self.bias)
        } else {
            self.bias
        }
    }

    fn append(&mut self, sample: f32, alpha: f32) {
        if self.bias.is_finite() && sample.is_finite() {
            self.bias = (sample - self.bias).mul_add(alpha, self.bias);
        } else {
            *self = Self { bias: sample, first: f32::NAN };
        }
    }

    fn join(self, after: Self, gain: f32) -> Self {
        if !after.first.is_finite() {
            after
        } else if !self.bias.is_finite() {
            Self { bias: after.seed(gain), first: f32::NAN }
        } else {
            Self { bias: gain.mul_add(self.bias, after.bias), first: self.first }
        }
    }
}

#[derive(Default)]
pub(super) struct Fold {
    points: [Vec<Point>; NCH],
    records: usize,
    gain: f64,
}

impl Fold {
    pub(super) fn reset(&mut self) {
        self.points.iter_mut().for_each(Vec::clear);
        self.records = 0;
        self.gain = 1.0;
    }

    pub(super) fn push(&mut self, channels: &[Vec<f32>; NCH], count: u32) {
        let alpha = 1.0 / count as f32;
        self.gain *= 1.0 - f64::from(alpha);
        for (points, values) in self.points.iter_mut().zip(channels) {
            if points.len() == values.len() {
                for (point, &sample) in points.iter_mut().zip(values) {
                    point.append(sample, alpha);
                }
            } else {
                points.clear();
                points.extend(values.iter().map(|&sample| {
                    if self.records == 0 && sample.is_finite() {
                        Point { bias: sample * alpha, first: sample }
                    } else {
                        Point { bias: sample, first: f32::NAN }
                    }
                }));
            }
        }
        self.records += 1;
    }

    pub(super) fn join(&mut self, after: &Self) {
        let gain = after.gain as f32;
        for (points, next) in self.points.iter_mut().zip(&after.points) {
            if self.records == 0 {
                points.clone_from(next);
            } else if points.len() != next.len() {
                points.clear();
                points.extend(next.iter().map(|p| Point { bias: p.seed(gain), first: f32::NAN }));
            } else {
                for (p, &q) in points.iter_mut().zip(next) {
                    *p = p.join(q, gain);
                }
            }
        }
        self.records += after.records;
        self.gain *= after.gain;
    }

    fn apply(&self, previous: Option<&Record>, out: &mut [Vec<f32>; NCH]) {
        let gain = self.gain as f32;
        for (c, (points, values)) in self.points.iter().zip(out).enumerate() {
            let prev = previous.map(|rec| &rec.ch[c]).filter(|ch| ch.len() == points.len());
            values.clear();
            values.extend(points.iter().enumerate().map(|(i, point)| {
                match prev.map(|ch| ch[i]).filter(|x| x.is_finite() && point.first.is_finite()) {
                    Some(y) => gain.mul_add(y, point.bias),
                    None => point.seed(gain),
                }
            }));
        }
    }
}

type FreeResults = VecDeque<Box<BatchResult>>;

#[derive(Default)]
pub(super) struct Storage(Arc<Mutex<FreeResults>>);

impl Storage {
    pub(super) fn take(&self, counters: &'static Counters) -> Output {
        let mut data = oos_hal::sync::lock(&self.0).pop_back().unwrap_or_default();
        data.fold.reset();
        data.pending = 0;
        data.context = Some((self.0.clone(), counters));
        Output(Some(data))
    }
}

#[derive(Default)]
struct BatchResult {
    fold: Fold,
    trigger: (f64, bool),
    pending: u64,
    timing: Option<(Option<Identity>, clock::StageClock)>,
    grid: (usize, f64, f64),
    context: Option<(Arc<Mutex<FreeResults>>, &'static Counters)>,
}

/// The batch's screen transform, with the same lifetime and FIFO order as its raw tap.
// The projection enum holds this handle without growing raw-only taps.
pub(crate) struct Output(Option<Box<BatchResult>>);

impl Output {
    fn data(&self) -> &BatchResult {
        self.0.as_deref().expect("the output owns its result until drop")
    }

    fn data_mut(&mut self) -> &mut BatchResult {
        self.0.as_deref_mut().expect("the output owns its result until drop")
    }

    pub(super) fn join(&mut self, fold: &Fold) {
        self.data_mut().fold.join(fold);
    }

    pub(super) fn finish(&mut self, trigger: (f64, bool), grid: (usize, f64, f64)) {
        let data = self.data_mut();
        data.trigger = trigger;
        data.grid = grid;
        data.pending = data.fold.records as u64;
    }

    pub(super) fn publish_clock(&mut self, id: Option<Identity>, mut clock: clock::StageClock) {
        clock.publish();
        self.data_mut().timing = Some((id, clock));
    }

    pub(crate) fn record(&self, frame: &Arc<Frame>) -> Record {
        let (points, window, first) = self.data().grid;
        let (ti, triggered) = self.data().trigger;
        let rec = Record {
            t0: first,
            dt: window / points as f64,
            triggered,
            partial: frame.partial,
            ch: Default::default(),
            raw: None,
            la: None,
            pairs: false,
        };
        crate::board::attach_raw(rec, frame, (window, first, ti))
    }

    pub(crate) fn apply(&self, previous: Option<&Record>, out: &mut [Vec<f32>; NCH]) {
        self.data().fold.apply(previous, out);
    }

    pub(crate) fn capacity(&self) -> usize {
        let points = self.data().fold.points.iter().map(Vec::capacity).sum::<usize>();
        points * (size_of::<Point>() / size_of::<f32>()) + METADATA_SAMPLES
    }

    pub(crate) fn shown(mut self) -> (usize, Option<Instant>) {
        let records = std::mem::take(&mut self.data_mut().pending);
        let data = self.data_mut();
        let counters = data.context.as_ref().expect("a live output belongs to a pool").1;
        let read_at = data.finish_picture(counters, true);
        if read_at.is_some() {
            counters.publish(&Default::default(), records);
        }
        (records as usize, read_at)
    }
}

impl BatchResult {
    fn finish_picture(&mut self, counters: &Counters, accepted: bool) -> Option<Instant> {
        let (identity, clock) = self.timing.take()?;
        let mut clocks = clock::BatchClocks::default();
        clocks.push(clock);
        let now = Instant::now();
        clocks.claimed(now);
        counters.metrics.finish(identity, &clocks, now, accepted);
        Some(clock.read_at())
    }
}

impl Drop for Output {
    fn drop(&mut self) {
        let Some(mut data) = self.0.take() else { return };
        let (home, counters) = data.context.take().expect("a live output belongs to a pool");
        if data.pending != 0 {
            counters.discard(Lost::Settings, data.pending);
            data.finish_picture(counters, false);
        }
        oos_hal::sync::lock(&home).push_back(data);
    }
}

pub(crate) fn software(m: &crate::model::Model) -> bool {
    m.acq == crate::model::AcqMode::Average && !m.avg_hw
}

#[cfg(any(target_os = "linux", test))]
impl super::ViewState {
    pub(super) fn average_samples(&self, id: oos_core::change::Identity) -> usize {
        usize::from(super::same_pair(self.identity, Some(id)) && self.view.as_ref().is_some_and(|v| software(&v.m)))
            * RESERVED_SAMPLES
    }
}

#[cfg(any(target_os = "linux", test))]
pub(crate) fn samples_for(pipe: &super::Pipe, id: oos_core::change::Identity) -> usize {
    oos_hal::sync::lock(&pipe.view).average_samples(id)
}

pub(crate) fn geometry_ok(pipe: &super::Pipe, id: oos_core::change::Identity, dt: f64, pairs: bool) -> bool {
    let state = oos_hal::sync::lock(&pipe.view);
    !super::same_pair(state.identity, Some(id))
        || state.view.as_ref().is_none_or(|v| !software(&v.m) || accepts_grid(&v.m, dt, pairs))
}

pub(super) fn accepts_grid(m: &crate::model::Model, dt: f64, pairs: bool) -> bool {
    let (points, window, _) = crate::board::record::hw_grid(m);
    !pairs && window / points as f64 / dt < 2.0
}

pub(crate) fn record_fallback(m: &crate::model::Model, rec: &Record) {
    use super::metrics::fallback::Cause;
    let cause = if software(m) && rec.raw.as_ref().is_some_and(|raw| !accepts_grid(m, raw.frame.dt, raw.frame.envelope))
    {
        Cause::AveragePairs
    } else {
        Cause::DirectRecord
    };
    super::counters().routes.fallbacks.add(cause.bit());
}
