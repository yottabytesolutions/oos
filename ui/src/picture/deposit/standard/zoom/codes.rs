// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::drawpipe::stage::fixture_stage;
use crate::fastdraw::{Codes, Samples};
use oos_dsp::interp::{linear_pair, Lanczos8Grid};
use std::ops::Range;

pub(super) mod detail;
mod shared;

#[derive(Default)]
pub(crate) struct ZoomScratch {
    overview: GridCache,
    detail: GridCache,
    support: Vec<f32>,
    pub(crate) lines: crate::fastdraw::zoom::Scratch,
    pub(super) filter: crate::fastdraw::zoom::Filter,
}

/// Place the Normal screen grids from their own BatchMeta-backed sample windows.
pub(crate) fn zoom_code_plans(
    m: &Model,
    frame: &Frame,
    (main, detail, legacy_trigger): (Projection, Projection, bool),
    samples: [Codes<'_>; NCH],
    (main_out, detail_out): (&mut [Vec<f32>; NCH], &mut [Vec<f32>; NCH]),
    previous_detail: Option<RawTrace>,
    refine_time: &mut Duration,
) -> Option<(FastFrame, FastFrame, f64)> {
    let dt = frame.dt;
    let hardware = hardware_position(m, frame);
    if m.acq != AcqMode::Normal
        || frame.envelope
        || frame.roll
        || frame.scan
        || !dt.is_finite()
        || dt <= 0.0
        || (hardware.is_none() && (m.trig_type != TrigType::Edge || m.trig_line().is_some()))
        || samples.iter().filter(|c| c.n > 0).any(|c| {
            let (low, high) = c.calibrated(0, u16::MAX);
            !low.is_finite() || !high.is_finite()
        })
    {
        Cause::ZoomRecordUnsupported.mark();
        return None;
    }
    let len = samples.iter().map(Samples::len).max().unwrap_or(0);
    let meta = frame_timing(m, frame);
    let (ti, outcome) =
        zoom_trigger(m, frame, main, (samples[m.trig_src], len), (legacy_trigger, hardware), &meta, refine_time)?;
    let triggered = outcome.triggered(&meta);
    let (overview_align, detail_align) = align(main, ti, dt, len);
    Some(crate::fastdraw::with_scratch(|scratch| {
        let ZoomScratch { overview, detail: detail_cache, support, filter, .. } = &mut scratch.routing.zoom;
        let mut place = |projection, alignment, out, cache: &mut GridCache| {
            captured_codes(m, frame, (projection, alignment, triggered), &samples, out, cache, support)
        };
        let overview = place(main, overview_align, main_out, overview);
        let detail =
            if detail.x_per_sample > 0.5 && detail.code_geometry() && !m.dots_at(detail.cols as f64 / POINTS as f64) {
                detail::place(
                    m.deskew_ch,
                    detail,
                    (detail_align.trigger, outcome),
                    &meta,
                    &samples,
                    filter,
                    previous_detail,
                )
            } else {
                place(detail, detail_align, detail_out, detail_cache)
            };
        (overview, detail, ti)
    }))
}

fn zoom_trigger(
    m: &Model,
    frame: &Frame,
    main: Projection,
    (source, len): (Codes<'_>, usize),
    (legacy_trigger, hardware): (bool, Option<f64>),
    meta: &FrameTiming,
    refine_time: &mut Duration,
) -> Option<(f64, kernel::TrigOutcome)> {
    if legacy_trigger {
        // Headered zoom used the record's screen-fit trigger before the layout gained a payload offset.
        let ti = Cause::ZoomHeaderTriggerMiss.absent(hardware.or_else(|| {
            let (span, first) = window(main);
            let src = (source.n > 1).then_some(&source)?;
            let geometry = (POINTS, span, first);
            let near = crate::deepmeas::deep_raw_frame(frame);
            Cause::ZoomHeaderTrigger.mark();
            fixture_stage!(Trigger, record::smoothed_edge(frame, m, src, len, geometry, near))
        }))?;
        return Some((ti, kernel::TrigOutcome::Hardware));
    }
    let route = route::route(frame, &main, m.acq, true);
    Some(batch_trigger(m, frame, &main, meta, (&SamplesRef::Codes(source), route), Some(refine_time)))
}

fn captured_codes(
    model: &Model,
    frame: &Frame,
    (projection, alignment, triggered): (Projection, Alignment, bool),
    samples: &[Codes<'_>; NCH],
    out: &mut [Vec<f32>; NCH],
    cache: &mut GridCache,
    support: &mut Vec<f32>,
) -> FastFrame {
    let dt = frame.dt;
    captured(
        model,
        frame,
        projection,
        alignment,
        triggered,
        std::mem::take(out),
        |(n, output_dt), windows, channels| {
            let (first, ti) = windows[0];
            let key = ScreenGrid { ti, first, dt: output_dt, sample_dt: dt, n, len: samples[0].n };
            if output_dt / dt >= 2.0
                && windows.iter().all(|&x| x == (first, ti))
                && shared::append(&mut cache.envelope[0], key, samples, channels)
            {
                Cause::ZoomSharedExtrema.mark();
                return;
            }
            for (c, (out, (first, ti))) in channels.iter_mut().zip(windows).enumerate() {
                let slot = if model.deskew_ch[c] == 0.0 { 0 } else { c + 1 };
                cache.resample(slot, &samples[c], dt, (n, output_dt, first), ti, (support, out));
            }
        },
    )
}

#[derive(Default)]
struct GridCache {
    linear: [LinearGrid; NCH + 1],
    sinc: [Lanczos8Grid; NCH + 1],
    envelope: [EnvelopeGrid; NCH + 1],
}

impl GridCache {
    fn resample(
        &mut self,
        slot: usize,
        codes: &Codes<'_>,
        sample_dt: f64,
        (n, dt, first): (usize, f64, f64),
        ti: f64,
        (support, out): (&mut Vec<f32>, &mut Vec<f32>),
    ) {
        if codes.n < 2 {
            out.clear();
            return;
        }
        let key = ScreenGrid { ti, first, dt, sample_dt, n, len: codes.n };
        if dt / sample_dt >= 2.0 {
            Cause::ZoomLaneExtrema.mark();
            let grid = &mut self.envelope[slot];
            grid.prepare(key);
            fixture_stage!(Resample, grid.append(codes, out));
        } else if sample_dt > 2.0 * dt {
            Cause::ZoomSinc.mark();
            let grid = &mut self.sinc[slot];
            let needed = grid.prepare((ti + first / sample_dt, dt / sample_dt), n, codes.n);
            decode(codes, needed, n, support);
            fixture_stage!(Resample, append_sinc(grid, support, out));
        } else {
            Cause::ZoomLinear.mark();
            let grid = &mut self.linear[slot];
            grid.prepare(key);
            fixture_stage!(Resample, grid.append(codes, support, out));
        }
    }
}

fn append_sinc(grid: &Lanczos8Grid, support: &[f32], out: &mut Vec<f32>) {
    out.clear();
    grid.append(support, out);
}

fn decode(codes: &Codes<'_>, range: Range<usize>, capacity: usize, out: &mut Vec<f32>) {
    out.clear();
    // A record clipped at the edge may need less support than the next record in the same view.
    out.reserve(capacity);
    out.extend(range.map(|i| codes.at(i)));
}

#[derive(Clone, Copy, PartialEq)]
struct ScreenGrid {
    ti: f64,
    first: f64,
    dt: f64,
    sample_dt: f64,
    n: usize,
    len: usize,
}

#[derive(Default)]
struct LinearGrid {
    geometry: Option<ScreenGrid>,
    points: Vec<(usize, f32)>,
    support: Range<usize>,
}

const OUTSIDE: usize = usize::MAX;

impl LinearGrid {
    fn prepare(&mut self, key: ScreenGrid) {
        if self.geometry == Some(key) {
            return;
        }
        self.geometry = Some(key);
        self.points.clear();
        let (mut lo, mut hi) = (key.len, 0);
        self.points.extend((0..key.n).map(|k| {
            let x = key.ti + (key.first + k as f64 * key.dt) / key.sample_dt;
            if !(0.0..=(key.len - 1) as f64).contains(&x) {
                return (OUTSIDE, 0.0);
            }
            let i = (x as usize).min(key.len - 2);
            lo = lo.min(i);
            hi = hi.max(i + 2);
            (i, (x - i as f64) as f32)
        }));
        self.support = lo.min(hi)..hi;
    }

    fn append(&self, codes: &Codes<'_>, support: &mut Vec<f32>, out: &mut Vec<f32>) {
        out.clear();
        let count = self.support.len();
        if count <= self.points.len() {
            decode(codes, self.support.clone(), self.points.len(), support);
            let base = self.support.start;
            self.interpolate(|i| support[i - base], out);
        } else {
            self.interpolate(|i| codes.at(i), out);
        }
    }

    fn interpolate(&self, at: impl Fn(usize) -> f32, out: &mut Vec<f32>) {
        out.extend(self.points.iter().map(|&(i, fr)| {
            if i == OUTSIDE {
                f32::NAN
            } else {
                linear_pair(at(i), at(i + 1), fr).unwrap_or(f32::NAN)
            }
        }));
    }
}

#[derive(Default)]
struct EnvelopeGrid {
    geometry: Option<ScreenGrid>,
    ranges: Vec<Range<usize>>,
}

impl EnvelopeGrid {
    fn prepare(&mut self, key: ScreenGrid) {
        if self.geometry == Some(key) {
            return;
        }
        self.geometry = Some(key);
        self.ranges.clear();
        let per = key.dt / key.sample_dt;
        // These overlapping pairs are the legacy screen envelope, spaced every two screen points.
        self.ranges.extend((0..key.n).step_by(2).map(|k| {
            let x = key.ti + (key.first + k as f64 * key.dt + 0.5 * key.dt) / key.sample_dt;
            if x < -per || x >= key.len as f64 + per {
                return 0..0;
            }
            let start = (x - per).max(0.0) as usize;
            let end = ((x + per).ceil() as usize).min(key.len);
            start.min(end)..end
        }));
    }

    fn append(&self, codes: &Codes<'_>, out: &mut Vec<f32>) {
        out.clear();
        out.extend(self.ranges.iter().flat_map(|range| {
            if range.is_empty() {
                [f32::NAN; 2]
            } else {
                let (lo, hi) = codes.extremes(range.start, range.end);
                [lo, hi]
            }
        }));
        if let Some(key) = self.geometry {
            out.truncate(key.n);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corrected_grids_keep_decoded_samples_at_the_edges_and_after_the_trigger_moves() {
        let words: Vec<u16> = (0..384).map(|i| 20_000 + (i * 131 % 24_173) as u16).collect();
        let bytes: Vec<u8> = words.iter().flat_map(|x| x.to_le_bytes()).collect();
        let offsets = [17, -82, 5, 43, -53, 12, 29, -1];
        let scale = 2.3;
        let offset = -0.37;
        let mut layout =
            crate::drawpipe::CodeLayout::uncorrected(1, [Some(0), None, None, None], [(0.0, 0.0); NCH], |_, _| {
                panic!("this fixture requires the corrected calibration")
            });
        layout.corrected = Some([crate::fastdraw::CorrectedCodes { offsets, scale, offset }; NCH]);
        let codes = layout.channel(&bytes, 0);
        let values: Vec<f32> = words
            .iter()
            .enumerate()
            .map(|(i, &raw)| {
                let code = (i32::from(raw) - offsets[i % 8]).clamp(0, 65535);
                ((f64::from(code) - 32768.0) / 7500.0 * scale - offset) as f32
            })
            .collect();
        let (mut cached, mut support, mut out, mut reference) =
            (GridCache::default(), Vec::new(), Vec::new(), Vec::new());
        let sample_dt = 3.2e-9;
        for step in [0.004_34, 0.868, 1.2, 8.68] {
            let grid = (POINTS, step * sample_dt, -8.0 * sample_dt);
            for trigger in [0.37, 153.416, 383.42, 383.421] {
                cached.resample(0, &codes, sample_dt, grid, trigger, (&mut support, &mut out));
                record::resample_channel(&values, (sample_dt, false), AcqMode::Normal, grid, trigger, &mut reference);
                assert_eq!(out.len(), reference.len());
                assert!(out
                    .iter()
                    .zip(&reference)
                    .all(|(a, b)| a.to_bits() == b.to_bits() || a.is_nan() && b.is_nan()));
            }
        }
    }
}
