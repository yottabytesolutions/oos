// SPDX-License-Identifier: GPL-2.0-or-later

//! Placement and column differences, independent of live settings and decoded frame ownership.

use crate::fastdraw::{self, Fold, Map, Samples, Spans};
use oos_core::model::{HDIV, NCH, VDIV};

/// Pairs retain the existing screen-record adapter until F5.
pub(crate) enum SamplesRef<'a> {
    Codes(fastdraw::Codes<'a>),
    Volts(&'a [f32]),
}

impl Samples for SamplesRef<'_> {
    fn len(&self) -> usize {
        match self {
            Self::Codes(v) => v.len(),
            Self::Volts(v) => v.len(),
        }
    }

    #[inline]
    fn at(&self, i: usize) -> f32 {
        match self {
            Self::Codes(v) => v.at(i),
            Self::Volts(v) => v.at(i),
        }
    }

    #[inline]
    fn extremes(&self, a: usize, b: usize) -> (f32, f32) {
        match self {
            Self::Codes(v) => v.extremes(a, b),
            Self::Volts(v) => v.extremes(a, b),
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Vertical {
    pub(crate) volts_per_div: f32,
    pub(crate) offset: f32,
}

/// Preserve the existing projection arithmetic in F1 so boundary pixels round identically.
#[derive(Clone, Copy)]
pub(crate) struct Projection {
    pub(crate) cols: usize,
    pub(crate) rows: usize,
    pub(crate) time_per_div: f64,
    pub(crate) x_per_sample: f64,
    pub(crate) delay: f64,
    pub(crate) ch: [Option<Vertical>; NCH],
    pub(crate) dots: bool,
}

impl Projection {
    pub(crate) fn record_rows(&self, c: usize, values: &[f32]) -> (usize, usize) {
        let Some(ch) = self.ch[c] else { return (usize::MAX, 0) };
        let h = self.rows as f32;
        let (lo, hi) = values
            .iter()
            .filter(|x| x.is_finite())
            .fold((f32::INFINITY, f32::NEG_INFINITY), |(a, b), &x| (a.min(x), b.max(x)));
        if lo > hi {
            return (usize::MAX, 0);
        }
        let y = |val: f32| h / 2.0 - (val + ch.offset) / ch.volts_per_div * (h / VDIV);
        let (a, b) = (y(hi).round() - 1.0, y(lo).round() + 1.0);
        (a.max(0.0) as usize, (b.max(-1.0) + 1.0).min(h) as usize)
    }
}

#[derive(Clone, Copy)]
pub(crate) struct TrigSnapshot {
    pub(crate) source: usize,
    pub(crate) hysteresis: f32,
    pub(crate) rising: bool,
    pub(crate) either: bool,
    pub(crate) on_la: bool,
    pub(crate) filtered: bool,
}

pub(crate) struct FrameTiming {
    pub(crate) dt: f64,
    pub(crate) pre: usize,
    pub(crate) trigger: TrigSnapshot,
}

#[derive(Debug, PartialEq)]
pub(crate) enum TrigOutcome {
    Hardware,
    AtPoint,
    WrongSlope,
    Moved(f64),
    NotFound,
}

impl TrigOutcome {
    pub(crate) fn edge_index(&self) -> Option<usize> {
        match self {
            Self::Hardware => None,
            Self::AtPoint => Some(0),
            Self::WrongSlope => Some(1),
            Self::Moved(_) => Some(2),
            Self::NotFound => Some(3),
        }
    }

    pub(crate) fn triggered(&self, meta: &FrameTiming) -> bool {
        !matches!(self, Self::NotFound) || meta.trigger.filtered
    }
}

pub(crate) struct RawTrace {
    pub(crate) trigger_at: f64,
    pub(crate) outcome: TrigOutcome,
    pub(crate) spans: [Option<Spans>; NCH],
}

pub(crate) struct Placed {
    pub(crate) projection: Projection,
    pub(crate) trace: RawTrace,
    pub(crate) fold: [Option<Fold>; NCH],
}

pub(crate) fn raw_map(proj: &Projection, c: usize, ti: f64) -> Option<Map> {
    let ch = proj.ch[c]?;
    let (w, h) = (proj.cols as f64, proj.rows as f32);
    let ppd_y = h / VDIV;
    Some(Map {
        ti,
        x_per_sample: proj.x_per_sample,
        x0: w / 2.0 - proj.delay / proj.time_per_div * w / HDIV as f64,
        y_per_volt: -ppd_y / ch.volts_per_div,
        y0: h / 2.0 - ch.offset / ch.volts_per_div * ppd_y,
    })
}

pub(crate) fn refine(
    meta: &FrameTiming,
    col: usize,
    search: impl FnOnce() -> (Option<f64>, Option<oos_dsp::edges::Edge>),
) -> (f64, TrigOutcome) {
    if meta.trigger.on_la {
        return (meta.pre as f64, TrigOutcome::Hardware);
    }
    let (edge, any) = search();
    let outcome = match (edge, any) {
        (None, _) => TrigOutcome::NotFound,
        (Some(t), _) if (t - meta.pre as f64).abs() <= (col + 16) as f64 => TrigOutcome::AtPoint,
        (Some(_), Some(n)) if !meta.trigger.either && n.rising != meta.trigger.rising => TrigOutcome::WrongSlope,
        (Some(t), _) => TrigOutcome::Moved((t - meta.pre as f64).abs()),
    };
    (edge.unwrap_or(meta.pre as f64), outcome)
}

pub(crate) fn place(
    samples: [SamplesRef<'_>; NCH],
    meta: &FrameTiming,
    proj: &Projection,
    level: f32,
    want_fold: [bool; NCH],
) -> Placed {
    let window = proj.time_per_div * HDIV as f64;
    let t_first = proj.delay - window / 2.0;
    let col = (1.0 / proj.x_per_sample).floor().clamp(1.0, 256.0) as usize;
    let t = meta.trigger;
    let (ti, outcome) = refine(meta, col, || {
        fastdraw::edge_smooth(
            &samples[t.source],
            meta.pre,
            crate::fastdraw::EdgeSearch { level, hysteresis: t.hysteresis, rising: t.rising, either: t.either },
            (window / 2.0 / meta.dt) as usize,
            col,
            |_| true,
        )
    });
    let used = |c: usize| proj.ch[c].is_some() && samples[c].len() > 0;
    let spans = std::array::from_fn(|c| {
        raw_map(proj, c, ti)
            .filter(|_| used(c) && !proj.dots)
            .map(|map| fastdraw::spans(proj.cols, proj.rows, &samples[c], &map))
    });
    let fold = std::array::from_fn(|c| {
        (want_fold[c] && used(c)).then(|| {
            let v = &samples[c];
            let n = v.len();
            let i0 = ((ti + t_first / meta.dt).floor().max(0.0) as usize).min(n);
            let i1 = ((ti + (t_first + window) / meta.dt).ceil().max(0.0) as usize).min(n);
            Fold::of(v, i0..i1.max(i0))
        })
    });
    Placed { projection: *proj, trace: RawTrace { trigger_at: ti, outcome, spans }, fold }
}

pub(crate) fn add_placed_band(
    c: usize,
    band: &mut [u16],
    c_lo: usize,
    samples: &SamplesRef<'_>,
    proj: &Projection,
    p: &RawTrace,
) {
    let Some(map) = raw_map(proj, c, p.trigger_at) else { return };
    match &p.spans[c] {
        Some(s) => fastdraw::add_spans_t(band, proj.rows, c_lo, s, 24),
        None => fastdraw::deposit_dots_t(band, proj.rows, c_lo, samples, &map, 24),
    }
}
