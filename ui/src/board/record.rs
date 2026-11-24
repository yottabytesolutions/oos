// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! A frame of the hardware made into a screen record: software trigger near the hardware trigger point, then the
//! screen window. Board-neutral: it works on frames in volts.

use super::Frame;
use crate::model::*;
use std::sync::Arc;

/// Maximum hardware records consumed by the UI in one step, before its time budget.
pub const RECORDS_PER_STEP: usize = 48;

/// The level the software trigger looks for on the samples of the source: the set level, and with AC or LF-reject
/// trigger coupling the level around the mean of the frame (the TPU filters the mean out before its comparators,
/// trigger.md 2.5; the frame holds the signal as the channel's own coupling gives it), from `MEAN_SAMPLES` samples.
pub fn sw_trig_level(m: &Model, f: &Frame) -> f32 {
    let v = &f.ch[m.trig_src.min(NCH - 1)];
    sw_trig_level_of(m, v.len(), |i| v[i])
}

/// `sw_trig_level` of a source of `len` samples read through `at` (the codes path reads the words): the same samples
/// in the same order, so the same level.
pub fn sw_trig_level_of(m: &Model, len: usize, at: impl Fn(usize) -> f32) -> f32 {
    match m.trig_coupling() {
        TrigCoup::Ac | TrigCoup::LfReject => m.trig_level + frame_mean(len, at).unwrap_or(0.0) as f32,
        TrigCoup::Dc | TrigCoup::HfReject => m.trig_level,
    }
}

/// Samples of the frame mean: the noise error is sigma/64. 1024 cost the same on the unit (trig-gate.md: the time is
/// not per sample), so the smaller error stays.
pub const MEAN_SAMPLES: usize = 4096;

/// The mean of the finite samples of `at` (0..`len`) at every `len / MEAN_SAMPLES`-th sample. A fixed stride: a
/// periodic signal not locked to it gives an error of about A/N. Locked (a period of step/j, at most a screen column)
/// it gives one fixed error, as every stride does; the column average of the edge search flattens such a signal, so
/// the level does not move the picture (docs/fork/trig-gate.md). A per-frame start or random offsets would turn it
/// into frame-to-frame noise of A/sqrt(2N) for every period below the step. None without a finite sample.
pub fn frame_mean(len: usize, at: impl Fn(usize) -> f32) -> Option<f64> {
    let step = (len / MEAN_SAMPLES).max(1);
    let (sum, n) = (0..len)
        .step_by(step)
        .map(at)
        .filter(|y| y.is_finite())
        .fold((0.0f64, 0u32), |(s, n), y| (s + y as f64, n + 1));
    (n > 0).then(|| sum / n as f64)
}

/// Use a fixed trigger when supplied, otherwise refine it with screen-point smoothing.
/// The raw deep window only uses the near search.
pub(crate) fn smoothed_edge(
    f: &Frame,
    m: &Model,
    src: &(impl crate::trignear::TriggerSamples + ?Sized),
    len: usize,
    (n, window, first): (usize, f64, f64),
    near_only: bool,
) -> Option<f64> {
    f.fixed_ti.or_else(|| smoothed_edge_with(f, m, src, len, (n, window, first), near_only, None))
}

fn smoothed_edge_with(
    f: &Frame,
    m: &Model,
    src: &(impl crate::trignear::TriggerSamples + ?Sized),
    len: usize,
    (n, window, first): (usize, f64, f64),
    near_only: bool,
    wide: Option<&mut crate::trignear::WideRefine<'_>>,
) -> Option<f64> {
    let span = (window / f.dt).ceil() as usize / 2;
    let col = (window / n as f64 / f.dt).floor().clamp(1.0, 256.0) as usize;
    let (rising, either) = (m.slope != Slope::Fall, m.slope == Slope::Either);
    let fits = trigger_fits(f.dt, (window, first, len));
    let level = sw_trig_level_of(m, src.len(), |i| src.at(i));
    let hyst = m.trig_hyst_div() * m.ch[m.trig_src].volts_per_div();
    if near_only {
        let found =
            crate::trignear::edge_near_window(src, 0, src.len(), f.pre, level, hyst, rising, either, span, col, fits);
        return Some(found.and_then(|r| r.0).unwrap_or(f.pre as f64));
    }
    let trigger = crate::fastdraw::EdgeSearch { level, hysteresis: hyst, rising, either };
    let require_end_rail = !crate::deepmeas::deep_pairs(f);
    let found =
        crate::trignear::edge_smooth_near_with(src, (f.pre, span, col), trigger, &fits, require_end_rail, wide).0;
    // A filtered comparator can fire without a crossing at this estimated software level.
    found.or((m.trig_coupling() != TrigCoup::Dc).then_some(f.pre as f64))
}

fn trigger_fits(dt: f64, (window, first, len): (f64, f64, usize)) -> impl Fn(f64) -> bool {
    let (first, last) = (-first / dt, len as f64 - 1.0 - (first + window) / dt);
    move |t| first > last || (first..=last).contains(&t)
}

/// The software trigger of `to_record`: the event nearest the hardware trigger point as a fractional sample index
/// of `f`, None without one (or in roll and scan).
pub fn trigger_point(f: &Frame, m: &Model, n: usize, window: f64, t_first: f64) -> Option<f64> {
    trigger_point_with(f, m, (n, window, t_first), None)
}

fn trigger_point_with(
    f: &Frame,
    m: &Model,
    (n, window, t_first): (usize, f64, f64),
    wide: Option<&mut crate::trignear::WideRefine<'_>>,
) -> Option<f64> {
    if f.fixed_ti.is_some() {
        return f.fixed_ti;
    }
    if f.roll || f.scan {
        return None;
    }
    let len = f.ch.iter().map(Vec::len).max().unwrap_or(0);
    if m.trig_type == TrigType::Edge && m.trig_line().is_none() && f.ch[m.trig_src].len() > 1 {
        let smooth = smoothed_edge_with(
            f,
            m,
            f.ch[m.trig_src].as_slice(),
            len,
            (n, window, t_first),
            crate::deepmeas::deep_raw_frame(f),
            wide,
        );
        if smooth.is_some() {
            return smooth;
        }
    }
    // the software trigger looks only around the hardware trigger point: searching whole 100 k-point frames for
    // every record cost most of the frame time; the video trigger counts fields and keeps the whole frame
    let screen = (window / f.dt).ceil() as usize;
    // the hardware trigger already put the event near `pre`: look a tenth of a screen around it, plus the time the
    // trigger condition needs before its decision (pulse widths, timeouts, Nth-edge idle time)
    let near = (screen / 10).max(2000);
    let span = (m.trig_span() / f.dt).ceil().max(0.0) as usize;
    let (lo, hi) = if m.trig_type == TrigType::Video {
        (0, len)
    } else {
        (f.pre.saturating_sub(near + span), (f.pre + near).min(len))
    };
    let e: [f32; 0] = [];
    let refs: [&[f32]; NCH] =
        std::array::from_fn(|c| if f.ch[c].len() < hi || lo >= hi { &e[..] } else { &f.ch[c][lo..hi] });
    // the hysteresis of the hardware comparators (0.3 div, trigger.md 2.3): with less, noise on an edge of the other
    // direction counts as an edge, and a slow noisy sine is drawn now on one, now on the other edge
    let hyst: [f32; NCH] = std::array::from_fn(|c| m.trig_hyst_div() * m.ch[c].volts_per_div());
    let inp = oos_dsp::trigger::Input { ch: refs, dt: f.dt, hyst };
    let usable = m.trig_channels().iter().enumerate().all(|(c, &u)| !u || !refs[c].is_empty());
    if let Some(n) = m.trig_line() {
        // the hardware trigger on an LA line: the analog samples cannot refine it. Without any edge on the line the
        // frame came from the auto trigger.
        f.la.as_ref().filter(|l| crate::ui::la::line_moves(l, n)).map(|_| f.pre as f64)
    } else if m.bus_on_la() {
        // the matching field nearest the hardware trigger: the record moves to it, as with the channels
        f.la.as_ref().and_then(|l| {
            let p = crate::trig::decode_la(m, &l.words, l.dt).ok()?;
            let t = crate::trig::points(m, &p)
                .into_iter()
                .map(|t1| l.t0 + t1 * l.dt)
                .min_by(|a, b| a.abs().total_cmp(&b.abs()))?;
            Some(f.pre as f64 + t / f.dt)
        })
    } else if m.pat_on_la() {
        // a pattern over D lines: as above; the frame counts as triggered when the pattern occurs in it
        f.la.as_ref().filter(|l| crate::ui::la::pattern_at(&l.words, &m.pat_la, 0).is_some()).map(|_| f.pre as f64)
    } else if usable {
        event_near(f, m, (lo, near, screen / 2), (window, t_first, len), &inp)
    } else {
        None
    }
}

fn event_near(
    f: &Frame,
    m: &Model,
    (lo, near, half_screen): (usize, usize, usize),
    (window, first, len): (f64, f64, usize),
    input: &oos_dsp::trigger::Input<'_>,
) -> Option<f64> {
    let fits = trigger_fits(f.dt, (window, first, len));
    let distance = |t: f64| (t - f.pre as f64).abs();
    if m.trig_type != TrigType::Edge {
        let all = crate::trig::find_all(m, input);
        return all
            .iter()
            .map(|t| t + lo as f64)
            .filter(|&t| fits(t))
            .min_by(|&a, &b| distance(a).total_cmp(&distance(b)))
            .or_else(|| (!all.is_empty()).then_some(f.pre as f64));
    }
    let source = m.trig_src;
    let nearest = |samples: &[f32], base: usize| {
        let mut found = None;
        let mut any = false;
        oos_dsp::edges::visit_edges(samples, m.trig_level, input.hyst[source], |edge| {
            if m.slope == Slope::Either || edge.rising == (m.slope == Slope::Rise) {
                any = true;
                let time = base as f64 + edge.t;
                if fits(time) && found.is_none_or(|old| distance(time).total_cmp(&distance(old)).is_lt()) {
                    found = Some(time);
                }
            }
        });
        (found, any)
    };
    let (found, any) = nearest(input.ch[source], lo);
    if found.is_some() || near >= half_screen {
        return found.or(any.then_some(f.pre as f64));
    }
    let first = f.pre.saturating_sub(half_screen);
    let end = (f.pre + half_screen).min(len);
    let (wide, any_wide) = nearest(f.ch[source].get(first..end).unwrap_or_default(), first);
    wide.or((any || any_wide).then_some(f.pre as f64))
}

/// Turns the latest frame into a screen record: software trigger (oos-dsp) closest to the hardware trigger point,
/// then the screen window (`n` points over `window` s from `t_first` relative to the trigger) by linear interpolation.
pub fn to_record(f: &Arc<Frame>, m: &Model, n: usize, window: f64, t_first: f64) -> crate::wave::Record {
    let (ti, triggered) = record_trigger(f, m, n, window, t_first);
    resample_record(f, m, n, window, t_first, ti, triggered)
}

/// A temporary display record borrows acquisition samples and retains no raw owner.
pub fn display_record(f: &Frame, m: &Model, n: usize, window: f64, t_first: f64) -> crate::wave::Record {
    let (ti, triggered) = record_trigger(f, m, n, window, t_first);
    resample(f, m, n, window, t_first, ti, triggered)
}

pub(crate) fn record_trigger(f: &Frame, m: &Model, n: usize, window: f64, t_first: f64) -> (f64, bool) {
    record_trigger_with(f, m, (n, window, t_first), None)
}

pub(crate) fn record_trigger_with(
    f: &Frame,
    m: &Model,
    geometry: (usize, f64, f64),
    wide: Option<&mut crate::trignear::WideRefine<'_>>,
) -> (f64, bool) {
    let (_, window, t_first) = geometry;
    let found = trigger_point_with(f, m, geometry, wide);
    // roll: right-aligned like a chart recorder, the start of the stream may be left of the buffer (blank)
    let ti = if f.roll {
        let len = f.ch.iter().map(|v| v.len()).max().unwrap_or(0) as f64;
        len - 1.0 - (window / 2.0) / f.dt
    } else if f.scan {
        // scan: point 0 at the left edge of the screen
        -t_first / f.dt
    } else {
        found.unwrap_or(f.pre as f64)
    };
    (ti, found.is_some())
}

/// Rebuild a display window from the retained acquisition without retriggering.
/// `ti` is the original accepted trigger's fractional sample index.
pub fn resample_record(
    f: &Arc<Frame>,
    m: &Model,
    n: usize,
    window: f64,
    t_first: f64,
    ti: f64,
    triggered: bool,
) -> crate::wave::Record {
    let mut rec = resample(f, m, n, window, t_first, ti, triggered);
    let len = f.ch.iter().map(|v| v.len()).max().unwrap_or(0);
    // the frame's samples inside the screen window
    let (i0, i1, t0) = raw_window(ti, t_first, window, f.dt, len);
    // Keep the acquisition even for a window outside it: navigation can return.
    rec.raw = (len > 1).then(|| crate::wave::Raw { frame: f.clone(), i0, i1, t0, conditioned: true });
    rec
}

pub(crate) fn raw_window(ti: f64, t_first: f64, window: f64, dt: f64, len: usize) -> (usize, usize, f64) {
    let i0 = ((ti + t_first / dt).floor().max(0.0) as usize).min(len);
    let i1 = ((ti + (t_first + window) / dt).ceil().max(0.0) as usize).clamp(i0, len);
    (i0, i1, (i0 as f64 - ti) * dt)
}

#[cfg(test)]
thread_local! {
    static RESAMPLES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn resample_count() -> usize {
    RESAMPLES.with(std::cell::Cell::get)
}

fn resample(
    f: &Frame,
    m: &Model,
    n: usize,
    window: f64,
    t_first: f64,
    ti: f64,
    triggered: bool,
) -> crate::wave::Record {
    let mut ch = Default::default();
    let pairs = resample_channels(f, m, (n, window, t_first), ti, &mut ch);
    // LA: the same shift as the analog record (software trigger at `ti` instead of the hardware point `pre`)
    let la = f.la.as_ref().map(|l| crate::ui::la::LaRec { t0: l.t0 - (ti - f.pre as f64) * f.dt, ..l.clone() });
    crate::wave::Record {
        t0: t_first,
        dt: window / n.max(1) as f64,
        ch,
        triggered,
        partial: f.partial,
        raw: None,
        la,
        pairs,
    }
}

/// Screen samples only, with space supplied by the caller.
pub(crate) fn resample_channels(
    f: &Frame,
    m: &Model,
    (n, window, first): (usize, f64, f64),
    ti: f64,
    channels: &mut [Vec<f32>; NCH],
) -> bool {
    #[cfg(test)]
    RESAMPLES.with(|count| count.set(count.get() + 1));
    let dt = window / n.max(1) as f64;
    let pairs = dt / f.dt >= 2.0 && m.acq != AcqMode::HiRes;
    for (values, out) in f.ch.iter().zip(channels) {
        resample_channel(values, (f.dt, f.envelope), m.acq, (n, dt, first), ti, out);
    }
    pairs
}

pub(crate) fn resample_channel(
    values: &[f32],
    (sample_dt, envelope): (f64, bool),
    mode: AcqMode,
    (n, dt, first): (usize, f64, f64),
    ti: f64,
    out: &mut Vec<f32>,
) {
    if values.len() < 2 {
        out.clear();
        return;
    }
    let per = dt / sample_dt;
    if per >= 2.0 && mode != AcqMode::HiRes {
        crate::picture::deposit::envelope::adapt::pairs::screen_envelope(
            values,
            (sample_dt, per),
            (n, dt, first),
            ti,
            out,
        );
        return;
    }
    crate::drawpipe::stage::fixture_stage!(
        Resample,
        resample_volts(values, (sample_dt, envelope), (n, dt, first), ti, out)
    );
}

fn resample_volts(
    values: &[f32],
    (sample_dt, envelope): (f64, bool),
    (n, dt, first): (usize, f64, f64),
    ti: f64,
    out: &mut Vec<f32>,
) {
    let per = dt / sample_dt;
    out.clear();
    if per >= 2.0 {
        out.extend((0..n).map(|k| {
            let x = ti + (first + (k as f64 + 0.5) * dt) / sample_dt;
            let a = (x - per / 2.0).max(0.0) as usize;
            let b = ((x + per / 2.0).ceil().max(0.0) as usize).min(values.len());
            let (sum, count) = values[a.min(b)..b]
                .iter()
                .filter(|y| y.is_finite())
                .fold((0.0f64, 0u32), |(sum, count), &y| (sum + f64::from(y), count + 1));
            if count > 0 {
                (sum / f64::from(count)) as f32
            } else {
                f32::NAN
            }
        }));
    } else if !envelope && sample_dt > 2.0 * dt {
        oos_dsp::interp::lanczos8_grid(values, ti + first / sample_dt, dt / sample_dt, n, out);
    } else {
        out.extend((0..n).map(|k| {
            let x = ti + (first + k as f64 * dt) / sample_dt;
            oos_dsp::interp::linear(values, x).unwrap_or(f32::NAN)
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exact_mean(v: &[f32]) -> f64 {
        v.iter().map(|&y| y as f64).sum::<f64>() / v.len() as f64
    }

    // Break it catches: a frame mean from too few samples or a stride that leaves part of the frame out: more than
    // 0.1 div from the mean of all samples for full-screen signals (+-4 div at 1 V/div), every length.
    #[test]
    fn frame_mean_within_a_tenth_of_a_division_at_full_screen() {
        let tau = std::f64::consts::TAU;
        let mut worst: f64 = 0.0;
        for len in [12_500usize, 62_500, 100_000, 1_000_000] {
            let mut sigs: Vec<(String, Vec<f32>)> = Vec::new();
            for period in [37.3, 401.7, 1234.5, 9876.5, 33_333.3] {
                sigs.push((
                    format!("sine {}", period),
                    (0..len).map(|i| (4.0 * (i as f64 / period * tau + 0.3).sin()) as f32).collect(),
                ));
            }
            sigs.push((
                "square 30 %".into(),
                (0..len).map(|i| if (i as f64 / 5003.7).fract() < 0.3 { 4.0 } else { -4.0 }).collect(),
            ));
            let mut ripple: Vec<f32> = (0..len).map(|i| (1.5 + 2.0 * (i as f64 / 2777.0 * tau).sin()) as f32).collect();
            oos_dsp::sim::add_noise(&mut ripple, 1.0, 9);
            sigs.push(("ripple and noise".into(), ripple));
            for (name, v) in &sigs {
                let want = exact_mean(v);
                let got = frame_mean(v.len(), |i| v[i]).unwrap();
                worst = worst.max((got - want).abs());
                assert!((got - want).abs() <= 0.1, "{} len {}: {} vs {}", name, len, got, want);
            }
        }
        println!("worst {:.4} div", worst);
        // locked: a period of the step (one screen column at 100 k points) gives a fixed error up to the amplitude, but
        // the column average of the edge search flattens the signal: at any level in the error's range the edge found
        // (if one) stays within a column of the hardware point
        let (len, col) = (100_000usize, 103usize);
        let step = len / MEAN_SAMPLES;
        for period in [step as f64, step as f64 / 2.0, step as f64 + 0.5] {
            let v: Vec<f32> = (0..len).map(|i| (4.0 * (i as f64 / period * tau + 0.4).sin()) as f32).collect();
            for k in 0..=16 {
                let level = -4.0 + 0.5 * k as f32;
                if let Some(t) =
                    crate::trignear::edge_near(&v, 0, len, len / 2, level, 0.3, true, false, len / 2, col).0
                {
                    assert!(
                        (t - (len / 2) as f64).abs() <= col as f64,
                        "period {} level {}: edge at {}",
                        period,
                        level,
                        t
                    );
                }
            }
        }
        assert!(frame_mean(10, |_| f32::NAN).is_none());
    }

    #[test]
    fn high_res_averages_where_the_record_has_more_samples_than_the_screen() {
        // 100 k samples of noise around 1 V, 3600 points: about 28 samples behind each point
        let mut x = 12345u32;
        let v: Vec<f32> = (0..100_000)
            .map(|_| {
                x = x.wrapping_mul(1664525).wrapping_add(1013904223);
                1.0 + ((x >> 8) as f32 / (1 << 24) as f32 - 0.5) * 0.4
            })
            .collect();
        let window = 100_000e-9;
        let frame = Arc::new(Frame {
            seq: 1,
            dt: 1e-9,
            pre: 50_000,
            ch: [v, Vec::new(), Vec::new(), Vec::new()],
            roll: false,
            envelope: false,
            scan: false,
            partial: false,
            acq: (0.0, 0),
            stamp: 0,
            la: None,
            fixed_ti: None,
            hw_avg: false,
            pre_exact: false,
            cfg: 0,
            basic: Default::default(),
        });
        let rms = |v: &[f32]| (v.iter().map(|y| (y - 1.0) * (y - 1.0)).sum::<f32>() / v.len() as f32).sqrt();
        let mut m = Model::default();
        m.trig_level = 5.0; // nothing to trigger on: the window stays at the hardware point
        let normal = to_record(&frame, &m, 3600, window, -window / 2.0);
        m.acq = AcqMode::HiRes;
        let hires = to_record(&frame, &m, 3600, window, -window / 2.0);
        assert!(hires.ch[0].iter().all(|y| y.is_finite()));
        // uniform noise of 0.115 V rms over 28 samples gives 0.022 V
        assert!(
            rms(&hires.ch[0]) < 0.03 && rms(&hires.ch[0]) < 0.25 * rms(&normal.ch[0]),
            "{} {}",
            rms(&hires.ch[0]),
            rms(&normal.ch[0])
        );
        let mean = hires.ch[0].iter().sum::<f32>() / 3600.0;
        assert!((mean - 1.0).abs() < 0.01, "{}", mean);
    }

    #[test]
    fn the_trigger_keeps_the_screen_window_inside_the_frame() {
        // rising edges at 4600 and 5600; 5600 is nearer the hardware point, but the window would run off the frame
        let v: Vec<f32> =
            (0..10_000).map(|i| if (4600..5000).contains(&i) || i >= 5600 { 2.0 } else { -2.0 }).collect();
        let mut m = Model::default();
        m.trig_level = 0.0;
        let window = 9000e-9;
        let f = Frame {
            seq: 1,
            dt: 1e-9,
            pre: 5400,
            ch: [v, Vec::new(), Vec::new(), Vec::new()],
            roll: false,
            envelope: false,
            scan: false,
            partial: false,
            acq: (0.0, 0),
            stamp: 0,
            la: None,
            fixed_ti: None,
            hw_avg: false,
            pre_exact: false,
            cfg: 0,
            basic: Default::default(),
        };
        let r = to_record(&Arc::new(f), &m, 3600, window, -window / 2.0);
        assert!(r.triggered);
        assert!(r.ch[0].iter().all(|y| y.is_finite()), "{} gaps", r.ch[0].iter().filter(|y| !y.is_finite()).count());
    }
}

#[cfg(test)]
mod peak_detect {
    use super::*;
    #[test]
    fn peak_detect_keeps_a_spike_and_is_cheap() {
        // the hardware trigger point is the falling edge at 50 k
        let m = Model { slope: Slope::Fall, ..Model::default() };
        let mut v: Vec<f32> = (0..100_000).map(|k| if (k / 25_000) % 2 == 0 { 0.0 } else { 3.0 }).collect();
        v[12_345] = 2.0; // one-sample glitch on the low level
        let f = Arc::new(Frame {
            seq: 1,
            dt: 20e-9,
            pre: 50_000,
            ch: [v, Vec::new(), Vec::new(), Vec::new()],
            roll: false,
            envelope: false,
            scan: false,
            partial: false,
            acq: (0.0, 0),
            stamp: 0,
            la: None,
            fixed_ti: None,
            hw_avg: false,
            pre_exact: false,
            cfg: 0,
            basic: Default::default(),
        });
        let window = m.time_per_div() as f64 * 10.0; // 2 ms > record: whole record on screen
        let t = std::time::Instant::now();
        let mut r = to_record(&f, &m, 3600, window, -window / 2.0);
        for _ in 0..47 {
            r = to_record(&f, &m, 3600, window, -window / 2.0);
        }
        eprintln!("PERF 48 x to_record (100 k samples): {:.1} ms", t.elapsed().as_secs_f64() * 1e3);
        // measurements see every captured sample of the window, not the 3600 screen points
        let raw = r.raw.as_ref().unwrap();
        assert_eq!((raw.i0, raw.i1), (0, 100_000));
        let mx = r.ch[0].iter().copied().filter(|x| x.is_finite() && *x < 2.5).fold(0.0f32, f32::max);
        assert!((mx - 2.0).abs() < 1e-6, "glitch lost: max of the low part {}", mx);
    }
}

#[cfg(test)]
mod dots {
    use super::*;

    /// The probe square (1 kHz, 0 / 0.3 V) at `tdiv`, 100 k samples over the window, drawn with dots; returns the
    /// columns that hold a period's edge and how many of them are lit from the low to the high level.
    fn edge_columns(tdiv: f64, pairs: bool) -> (usize, usize) {
        let mut m = Model { dots: DotsMode::On, slope: Slope::Rise, trig_level: 0.15, ..Model::default() };
        m.tscale = tscales().iter().position(|&x| (x as f64 - tdiv).abs() < tdiv * 1e-3).unwrap();
        m.ch[0].offset = -0.15;
        m.ch[0].vscale = VSCALES.iter().position(|&x| (x - 0.1).abs() < 1e-6).unwrap();
        (m.ch[0].vfine, m.ch[0].probe) = (1.0, 1.0);
        let window = tdiv * HDIV as f64;
        let n = 100_000usize;
        let dt = window / n as f64;
        // 3 us edges, the trigger at the middle on a rising one
        let sig = |t: f64| {
            let ph = (t + 1.0).rem_euclid(1e-3);
            (if ph < 3e-6 {
                ph / 3e-6
            } else if ph < 0.5e-3 {
                1.0
            } else if ph < 0.5e-3 + 3e-6 {
                1.0 - (ph - 0.5e-3) / 3e-6
            } else {
                0.0
            }) as f32
                * 0.3
        };
        let v: Vec<f32> = (0..n).map(|k| sig((k as f64 - (n / 2) as f64) * dt)).collect();
        let f = Arc::new(Frame {
            seq: 1,
            dt,
            pre: n / 2,
            ch: [v, Vec::new(), Vec::new(), Vec::new()],
            ..Default::default()
        });
        let mut rec = to_record(&f, &m, 3600, window, -window / 2.0);
        assert!(rec.pairs);
        rec.pairs = pairs;
        let mut ph = crate::wave::Phosphor::new(1000, 480);
        ph.draw_ex(0, &m, (m.time_per_div(), m.delay), rec.t0, rec.dt, &rec.ch[0], 24, None, rec.pairs);
        let (w, h) = (ph.w, ph.h);
        let rows = |c: usize| (0..h).filter(|&r| ph.maps()[0][r * w + c] > 0).collect::<Vec<_>>();
        // the levels' rows from a column on a plateau: the first of the screen lies on the low level
        let (y_lo, y_hi) = (240.0 + 0.15 / 0.1 * 60.0, 240.0 - 0.15 / 0.1 * 60.0);
        let cols_per_s = w as f64 / window;
        let mut edges = 0;
        let mut lit = 0;
        for p in 0.. {
            let t = -window / 2.0 + p as f64 * 0.5e-3;
            if t > window / 2.0 {
                break;
            }
            // an edge at every half period; the column it falls in
            let c = ((t + window / 2.0) * cols_per_s).floor() as usize;
            if c == 0 || c + 1 >= w {
                continue;
            }
            edges += 1;
            let r: Vec<usize> = [c - 1, c, c + 1].iter().flat_map(|&c| rows(c)).collect();
            let span = |y: f64| r.iter().any(|&x| (x as f64 - y).abs() <= 2.0);
            if span(y_lo) && span(y_hi) && (y_hi as usize..y_lo as usize).all(|y| r.contains(&y)) {
                lit += 1;
            }
        }
        (edges, lit)
    }

    /// Dots on a peak-detect record: each min/max pair is drawn as one span, so the edges of a slow time base stay on
    /// the screen (docs/fork/zoomout.md). Two dots per pair left the edges blank from 2 ms/div on.
    #[test]
    fn dots_draw_min_max_pairs_as_spans() {
        for tdiv in [2e-3, 50e-3] {
            let (edges, lit) = edge_columns(tdiv, true);
            assert!(edges >= 20 && lit == edges, "{} s/div: {} of {} edges drawn", tdiv, lit, edges);
            let (_, lit_dots) = edge_columns(tdiv, false);
            assert!(lit_dots == 0, "{} s/div: the old dots drew {} edges", tdiv, lit_dots);
        }
    }
}

#[cfg(test)]
mod bench {
    use super::*;

    /// Per-record cost of the display path (`cargo test --release bench_record -- --ignored --nocapture`).
    #[test]
    #[ignore = "host timing: set OOS_PERF_TESTS=1"]
    fn bench_record() {
        assert_eq!(std::env::var("OOS_PERF_TESTS").as_deref(), Ok("1"), "set OOS_PERF_TESTS=1");
        for &(tdiv, pts) in &[(1e-7, 1408usize), (1e-7, 352), (1e-6, 12_500), (1e-5, 100_000)] {
            let mut m = Model::default();
            m.tscale = tscales().iter().position(|&x| (x as f64 - tdiv).abs() < tdiv * 1e-3).unwrap();
            m.trig_level = 0.0;
            m.ch[0].offset = 0.0;
            let dt = 0.8e-9 * (tdiv / 1e-7).max(1.0) * 1408.0 / pts as f64 * if pts == 1408 { 1.0 } else { 1.0 };
            let f = 3.0 / (tdiv * 10.0);
            let v: Vec<f32> = (0..pts).map(|k| (2.0 * std::f64::consts::PI * f * k as f64 * dt).sin() as f32).collect();
            let fr = Arc::new(Frame {
                seq: 1,
                dt,
                pre: pts / 2,
                ch: [v, Vec::new(), Vec::new(), Vec::new()],
                roll: false,
                envelope: false,
                scan: false,
                partial: false,
                acq: (1.0 / dt, pts),
                stamp: 0,
                la: None,
                fixed_ti: None,
                hw_avg: false,
                pre_exact: false,
                cfg: 0,
                basic: Default::default(),
            });
            let window = tdiv * 10.0;
            let mut ph = crate::wave::Phosphor::new(970, 448);
            let n = 400;
            let (mut t_rec, mut t_draw, mut t_clone) = (0.0, 0.0, 0.0);
            for _ in 0..n {
                let t0 = std::time::Instant::now();
                let rec = to_record(&fr, &m, 3600, window, -window / 2.0);
                let t1 = std::time::Instant::now();
                ph.draw_ex(0, &m, (tdiv as f32, 0.0), rec.t0, rec.dt, &rec.ch[0], 24, None, rec.pairs);
                let t2 = std::time::Instant::now();
                let h = crate::wave::Record { raw: None, ..rec.clone() };
                std::hint::black_box(&h);
                t_rec += (t1 - t0).as_secs_f64();
                t_draw += (t2 - t1).as_secs_f64();
                t_clone += t2.elapsed().as_secs_f64();
            }
            // the fast path: edge near pre, raw samples straight into the map
            let mut t_fast = 0.0;
            let mut map = vec![0u16; 970 * 448];
            for _ in 0..n {
                let t0 = std::time::Instant::now();
                let ti =
                    crate::fastdraw::edge_near(&fr.ch[0], fr.pre, 0.0, 0.0, true, false, 16).unwrap_or(fr.pre as f64);
                let xm = crate::fastdraw::Map {
                    ti,
                    x_per_sample: dt / window * 970.0,
                    x0: 485.0,
                    y_per_volt: -56.0,
                    y0: 224.0,
                };
                let _ = crate::fastdraw::deposit(&mut map, 970, 448, &fr.ch[0], &xm, 24);
                t_fast += t0.elapsed().as_secs_f64();
            }
            let us = |t: f64| t / n as f64 * 1e6;
            eprintln!(
                "bench {:>6} pts {:>8.0e} s/div: to_record {:7.1} us, draw {:6.1} us, clone {:5.1} us | fast {:6.1} us",
                pts,
                tdiv,
                us(t_rec),
                us(t_draw),
                us(t_clone),
                us(t_fast)
            );
        }
    }
}

#[cfg(test)]
mod pair_tests {
    use super::*;

    // Break it catches: the wide software trigger search run on a deep raw window (millions of samples, about 160 ms
    // on the unit per window with a flat source), or a near edge not taken. A deep raw window takes the near search
    // alone and the hardware point without an edge near it; the same frame not deep searches wide.
    #[test]
    fn a_deep_raw_window_searches_near_the_trigger_only() {
        let mut m = Model::default();
        m.tscale = tscales().iter().position(|&x| (x as f64 - 1e-3).abs() < 1e-9).unwrap();
        m.trig_level = 0.5;
        let (n_s, window) = (1_000_000usize, 1e-2);
        let dt = window / n_s as f64;
        let pre = n_s / 2;
        // one rising edge `at` samples from the hardware point
        let frame = |at: i64, depth: usize| {
            let v: Vec<f32> = (0..n_s).map(|i| if (i as i64) < pre as i64 + at { 0.0 } else { 1.0 }).collect();
            Frame {
                seq: 1,
                dt,
                pre,
                ch: [v, Vec::new(), Vec::new(), Vec::new()],
                acq: (1.0 / dt, depth),
                ..Default::default()
            }
        };
        let t = |f: &Frame| trigger_point(f, &m, 3600, window, -window / 2.0).unwrap();
        // near (100 samples): found either way
        let near_deep = t(&frame(100, 10_000_000));
        assert!((near_deep - (pre + 100) as f64).abs() < 300.0, "{}", near_deep);
        assert_eq!(near_deep, t(&frame(100, 1_000_000)));
        // far (200 k samples, beyond the near search): the wide search finds it, the deep window keeps the hardware point
        assert!((t(&frame(200_000, 1_000_000)) - (pre + 200_000) as f64).abs() < 300.0);
        assert_eq!(t(&frame(200_000, 10_000_000)), pre as f64);
        // flat: the hardware point
        let mut flat = frame(0, 10_000_000);
        flat.ch[0] = vec![0.0; n_s];
        assert_eq!(t(&flat), pre as f64);
    }

    // Break it catches: the NEON extremes of a pair span other than the filtered scalar loop's: a NaN gap that is not
    // skipped, an infinite sample that counts, a span left out. Every pair point of a dense record equals the
    // extremes of the finite samples of its span.
    #[test]
    fn pair_points_are_the_extremes_of_their_finite_samples() {
        let mut m = Model::default();
        m.tscale = tscales().iter().position(|&x| (x as f64 - 1e-3).abs() < 1e-9).unwrap();
        m.trig_level = 0.0;
        let n_s = 400_000usize;
        let dt = 1e-2 / n_s as f64;
        let mut x = 7u32;
        let v: Vec<f32> = (0..n_s)
            .map(|k| {
                x = x.wrapping_mul(1664525).wrapping_add(1013904223);
                match k % 997 {
                    0 => f32::NAN,
                    500 => f32::INFINITY,
                    501 => f32::NEG_INFINITY,
                    _ => (k as f32 * 3e-4).sin() + ((x >> 8) as f32 / (1u32 << 24) as f32 - 0.5) * 0.1,
                }
            })
            .collect();
        let fr = Arc::new(Frame {
            seq: 1,
            dt,
            pre: n_s / 2,
            ch: [v.clone(), Vec::new(), Vec::new(), Vec::new()],
            acq: (1.0 / dt, n_s),
            ..Default::default()
        });
        let (n, window) = (3600usize, 1e-2);
        let t_first = -window / 2.0;
        let rec = to_record(&fr, &m, n, window, t_first);
        assert!(rec.pairs);
        let ti = trigger_point(&fr, &m, n, window, t_first).unwrap_or(fr.pre as f64);
        let (rdt, per) = (window / n as f64, window / n as f64 / dt);
        for j in 0..n / 2 {
            let xm = ti + (t_first + (2 * j) as f64 * rdt + 0.5 * rdt) / dt;
            let a = (xm - per).max(0.0) as usize;
            let b = ((xm + per).ceil() as usize).min(v.len());
            let fin = v[a.min(b)..b].iter().copied().filter(|y| y.is_finite());
            let (lo, hi) = fin.fold((f32::INFINITY, f32::NEG_INFINITY), |(l, h), y| (l.min(y), h.max(y)));
            let got = (rec.ch[0][2 * j], rec.ch[0][2 * j + 1]);
            if lo > hi {
                assert!(got.0.is_nan() && got.1.is_nan(), "pair {} outside the frame: {:?}", j, got);
            } else {
                assert_eq!(got, (lo, hi), "pair {}", j);
            }
        }
    }

    #[test]
    fn borrowed_display_conversion_matches_retained_records_exactly() {
        for acq in [AcqMode::Normal, AcqMode::Peak, AcqMode::Average, AcqMode::HiRes] {
            for envelope in [false, true] {
                let mut m = Model { acq, ..Default::default() };
                for c in &mut m.ch {
                    c.on = true;
                }
                let f = Arc::new(Frame {
                    dt: 1e-6,
                    pre: 5000,
                    ch: std::array::from_fn(|c| (0..10_000).map(|i| ((i + c) as f32 * 0.01).sin()).collect()),
                    envelope,
                    acq: (1e6, 10_000_000),
                    ..Default::default()
                });
                for (window, first) in [(1e-2, -5e-3), (1e-3, -2e-3), (1e-3, 1.0)] {
                    let retained = to_record(&f, &m, 3600, window, first);
                    let borrowed = display_record(&f, &m, 3600, window, first);
                    assert!(borrowed.raw.is_none());
                    assert_eq!(
                        (retained.t0, retained.dt, retained.triggered, retained.pairs),
                        (borrowed.t0, borrowed.dt, borrowed.triggered, borrowed.pairs)
                    );
                    for (a, b) in retained.ch.iter().zip(&borrowed.ch) {
                        assert_eq!(
                            a.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                            b.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
                        );
                    }
                }
            }
        }
    }
}
