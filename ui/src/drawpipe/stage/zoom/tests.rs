// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::board::scripted::{self, Batch as _};
use std::hint::black_box;

const WIDTH: usize = 970;
const HEIGHT: usize = 224;

fn model(channels: usize) -> Model {
    let mut m = crate::drawpipe::tests::model(10e-6, false, false);
    for (c, channel) in m.ch.iter_mut().enumerate() {
        channel.on = c < channels;
    }
    m
}

fn view(mut m: Model, zoom_div: Option<f32>) -> Arc<View> {
    m.zoom = zoom_div.is_some();
    if let Some(tdiv) = zoom_div {
        m.zscale = tscales().iter().position(|&t| t == tdiv).unwrap();
    }
    Arc::new(View {
        gen: 0,
        m,
        interp: Interp::Exact,
        w: WIDTH,
        h: if zoom_div.is_some() { HEIGHT } else { 2 * HEIGHT },
        zoom: zoom_div.map(|_| route::ZoomView::new((WIDTH, HEIGHT), 1.0, 0.0)),
    })
}

fn batch(m: &Model) -> (Arc<Batch>, Frame) {
    let pipe = Pipe::start(0, 0, Box::leak(Box::default()));
    let switches = crate::sysconf::Switches { compress: false, ..Default::default() };
    let (input, frames, _) = scripted::draw_route_batch(m, &switches, 1);
    let frame = input.frame(0, 0);
    let (fb, decode, codes) = (input.frame_bytes(), input.decode(), input.codes());
    (pipe.feed_batch(Box::new(input.into_bytes()), fb, 0..frames, (0, None), decode, codes), frame)
}

fn prepared() -> Prepared {
    Prepared {
        fallbacks: Default::default(),
        index: 0,
        decoded_dots: false,
        plan: None,
        codes: None,
        folds: Vec::new(),
        measured: false,
        refine: Duration::ZERO,
        screen: Default::default(),
        zoom: None,
    }
}

#[test]
fn zoom_peak_pairs_with_statistics_do_not_decode() {
    use crate::drawpipe::tests::Bytes;

    let mut m = model(1);
    m.acq = AcqMode::Peak;
    m.meas = vec![(Meas::Vpp, WaveId::Ch(0))];
    let view = view(m, Some(1e-6));
    let frame_bytes = 2 * 8784;
    let codes: CodesInto = Arc::new(|_, _, frame| {
        frame.envelope = true;
        frame.fixed_ti = Some(4388.0);
        frame.pre = 4388;
        frame.acq = (78_125_000.0, 8784);
        frame.dt = 1.0 / frame.acq.0;
        let layout =
            CodeLayout::uncorrected(1, [Some(0), None, None, None], [(1.0, 0.0); NCH], |code, _| f32::from(code));
        Some(layout.with_reduced_capture(true))
    });
    let decode: DecodeInto = Arc::new(|_, _| panic!("zoom placement must not decode live Peak pairs"));
    let pipe = Pipe::start(0, 0, Box::leak(Box::default()));
    let batch =
        pipe.feed_batch(Box::new(Bytes(vec![0; frame_bytes])), frame_bytes, 0..1, (0, None), decode, Some(codes));
    let mut p = prepared();
    p.folds = crate::framestats::folds_for(&view.m);
    super::super::place(&batch, &view, &mut p, &mut Frame::default());
    assert!(p.plan.is_some() && p.zoom_plan().is_some());
    assert!(!p.measured);
    assert_eq!(p.folds[0].1.n, 0);
}

#[test]
fn zoom_seed_starts_with_enough_records_for_the_placement_workers() {
    use crate::drawpipe::admission::{model_pool, Grant};

    let pipe = Pipe::start(0, 0, Box::leak(Box::default()));
    model_pool(&pipe, 4);
    let v = view(model(4), Some(5e-9));
    let id = oos_core::change::Identity::default();
    pipe.publish_picture_view(&v.m, (v.w, v.h), v.interp, id, v.zoom);
    let budget = crate::drawpipe::budget::default_work_budget(Duration::from_micros(30_750));
    assert!(matches!(pipe.grant(id, 11, budget), Grant::Overlapping(4..)));
}

fn trigger_fixture(exact: bool) -> (Arc<Batch>, Arc<View>) {
    let mut m = model(4);
    m.trig_mode = TrigMode::Normal;
    m.trig_src = 1;
    m.trig_level = 4.93;
    m.ch[1].offset = -m.trig_level;
    m.meas = vec![(Meas::Vpp, WaveId::Ch(0)), (Meas::Vmax, WaveId::Ch(1))];
    let switches = crate::sysconf::Switches { compress: false, ..Default::default() };
    let (input, frames, _) = scripted::draw_route_batch(&m, &switches, 1);
    let (fb, decode, codes) = (input.frame_bytes(), input.decode(), input.codes());
    let mut bytes = input.into_bytes().bytes().to_vec();
    let mut frame = Frame::default();
    decode(&bytes, &mut frame);
    assert_eq!((frame.acq.0, frame.ch[1].len()), (312_500_000.0, 35_160));
    assert!(frame.trigger_anchor.at_adc_rate());
    let arm = (m.trig_hyst_div() * 7500.0).round() as u16;
    const PERIOD: usize = 3125;
    for (i, group) in bytes.as_chunks_mut::<8>().0.iter_mut().enumerate() {
        let phase = (i + PERIOD - frame.pre % PERIOD) % PERIOD;
        // The first rising edge crosses the level at pre and reaches the upper rail 300 samples later.
        let code = match phase {
            0..300 => 32_768 + arm / 2,
            300..1562 => 32_768 + 2 * arm,
            _ => 32_768 - 2 * arm,
        };
        group[2..4].copy_from_slice(&code.to_le_bytes());
    }
    let (decode, codes) = if exact {
        (decode, codes)
    } else {
        let raw = codes.unwrap();
        let codes: CodesInto = Arc::new(move |i, bytes, frame| {
            let layout = raw(i, bytes, frame)?;
            frame.trigger_anchor = crate::board::TriggerAnchor::new(false);
            Some(layout)
        });
        let decode: DecodeInto = Arc::new(move |bytes, frame| {
            decode(bytes, frame);
            frame.trigger_anchor = crate::board::TriggerAnchor::new(false);
        });
        (decode, Some(codes))
    };
    let pipe = Pipe::start(0, 0, Box::leak(Box::default()));
    let batch =
        pipe.feed_batch(Box::new(crate::drawpipe::tests::Bytes(bytes)), fb, 0..frames, (0, None), decode, codes);
    (batch, view(m, Some(5e-9)))
}

#[test]
fn zoom_uses_the_dense_narrow_trigger_at_10us_per_division() {
    for exact in [true, false] {
        check_narrow_trigger(exact);
    }
}

fn dense_placement(batch: &Arc<Batch>, zoom: &View) -> standard::FastFrame {
    let dense = view(zoom.m.clone(), None);
    let mut p = prepared();
    super::super::place(batch, &dense, &mut p, &mut Frame::default());
    p.plan.unwrap()
}

fn check_narrow_trigger(exact: bool) {
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let (batch, zoom) = trigger_fixture(exact);
    let plan = dense_placement(&batch, &zoom);
    assert!(plan.triggered);
    assert_eq!(plan.route, route::Route::Dense);
    let dense_ti = plan.plan.raw().unwrap().1.trigger_at;
    let routes = &crate::drawpipe::counters().routes;
    assert_eq!(routes.wide_searches(), 0);

    let (mut main, mut detail) = (Default::default(), Default::default());
    let mut scratch = Frame::default();
    let mut refine = Duration::ZERO;
    let mut place = |main: &mut [Vec<f32>; NCH], detail: &mut [Vec<f32>; NCH], previous| {
        place_codes(&batch, &zoom, (0, chunk(&batch, 0)), &mut scratch, (main, detail), &mut refine, (None, previous))
            .unwrap()
    };
    let (overview, closeup, _, _) = place(&mut main, &mut detail, None);
    overview.plan.recycle(&mut main);
    let previous = closeup.plan.recycle(&mut detail);
    let (allocations, (overview, closeup, _, ti)) =
        crate::system::test_allocator::count_own(|| place(&mut main, &mut detail, previous));
    assert_eq!(allocations, 0);
    assert_eq!(routes.wide_searches(), 0);
    assert_eq!(ti, dense_ti);
    assert!(overview.triggered && closeup.triggered);

    let mut measured = prepared();
    measured.folds = crate::framestats::folds_for(&zoom.m);
    super::super::place(&batch, &zoom, &mut measured, &mut scratch);
    for (_, fold) in &mut measured.folds {
        *fold = Default::default();
    }
    let (allocations, ()) =
        crate::system::test_allocator::count_own(|| super::super::place(&batch, &zoom, &mut measured, &mut scratch));
    assert_eq!(allocations, 0);
    assert!(measured.measured);
    assert_eq!(routes.wide_searches(), 0, "measurements reuse the accepted trigger");
    assert!(scratch.ch.iter().all(Vec::is_empty), "extrema stay on codes");
    let mut frame = Frame::default();
    batch.decode.decode(0, chunk(&batch, 0), &mut frame);
    let mut expected = crate::framestats::folds_for(&zoom.m);
    assert!(crate::framestats::fold_frame_with(&frame, &zoom.m, &mut expected, Some(ti)));
    assert_eq!(measured.folds, expected);
}

#[test]
fn raw_measurements_keep_the_fractional_trigger_time() {
    use oos_dsp::measure::{analyze, delay, phase, Pair};

    let mut m = model(2);
    m.trig_src = 0;
    m.trig_level = 0.0;
    m.slope = Slope::Rise;
    let pre = 2048;
    let period = 128;
    let lag = 11;
    let dt = 2.0f64.powi(-23);
    let signal = |offset| {
        (0..4096)
            .map(|i| if (i + period - (pre + 3 + offset) % period) % period < period / 2 { 1.0 } else { -1.0 })
            .collect()
    };
    let frame = Arc::new(Frame { pre, dt, ch: [signal(0), signal(lag), Vec::new(), Vec::new()], ..Default::default() });
    let window = f64::from(m.time_per_div()) * f64::from(HDIV);
    let record = crate::board::to_record(&frame, &m, crate::sparse::POINTS, window, -window / 2.0);
    assert!(record.triggered);
    let raw = record.raw.unwrap();
    let origin = raw.t0 - raw.i0 as f64 * dt;
    assert_eq!(origin, -(pre as f64 + 2.5) * dt);

    let (a, b) = (analyze(&raw.frame.ch[0], dt), analyze(&raw.frame.ch[1], dt));
    assert!(a.rises.iter().any(|&t| t + origin == 0.0));
    assert_eq!(delay(&a, &b, Pair(true, true)), lag as f64 * dt);
    assert_eq!(phase(&a, &b, Pair(true, true)), 360.0 * lag as f64 / period as f64);
}

#[test]
fn a_missing_edge_keeps_the_code_zoom_plan() {
    let (batch, mut zoom) = trigger_fixture(true);
    Arc::get_mut(&mut zoom).unwrap().m.trig_level = 20.0;
    let mut p = prepared();
    let mut scratch = Frame::default();
    super::super::place(&batch, &zoom, &mut p, &mut scratch);
    assert!(!p.plan.unwrap().triggered);
    assert!(!p.zoom.unwrap().plan.unwrap().triggered);
    assert!(scratch.ch.iter().all(Vec::is_empty));
}

fn median_us(mut work: impl FnMut()) -> f64 {
    const RUNS: usize = 1000;
    work();
    let mut runs: [f64; 7] = std::array::from_fn(|_| {
        let started = Instant::now();
        for _ in 0..RUNS {
            work();
        }
        started.elapsed().as_secs_f64() * 1e6 / RUNS as f64
    });
    runs.sort_by(f64::total_cmp);
    runs[runs.len() / 2]
}

#[test]
#[ignore = "release host placement benchmark"]
fn measured_zoom_10us_placement() {
    let (batch, view) = trigger_fixture(true);
    let mut p = prepared();
    p.folds = crate::framestats::folds_for(&view.m);
    let mut scratch = Frame::default();
    let bytes = chunk(&batch, 0);
    let us = median_us(|| {
        place(&batch, &view, black_box(&mut p), &mut scratch, black_box(bytes));
        black_box(&p.plan);
    });
    println!("4ch 10 us/div, zoom 5 ns/div, 35,160 samples/channel, Vpp/Vmax: {us:.3} us/record");
    report_trigger_pixel_difference();
}

fn report_trigger_pixel_difference() {
    let m = model(1);
    let (batch, frame) = batch(&m);
    let v = view(m.clone(), Some(1e-6));
    let legacy = standard::screen_trigger(&m, &frame, &route::projection(&v, frame.dt).0.main(), None, None).0;
    let shared = dense_placement(&batch, &v).plan.raw().unwrap().1.trigger_at;
    let view = (1e-6, m.zdelay);
    let before = reference_picture(&m, &frame, view, (legacy, legacy));
    let after = reference_picture(&m, &frame, view, (shared, shared));
    let differing_pixels = before.maps()[0].iter().zip(&after.maps()[0]).filter(|(a, b)| a != b).count();
    println!("Zoom pixel fixture: legacy trigger {legacy}, shared trigger {shared}, {differing_pixels} changed CH1 detail pixels");
}

#[test]
#[ignore = "release host placement benchmark"]
fn zoom_placement_report() {
    let m = model(4);
    let (b, frame) = batch(&m);
    println!("geometry rate={} stored={} words/channel={}", 1.0 / frame.dt, frame.acq.1, frame.ch[0].len());
    let mut scratch = Frame::default();
    let bytes = chunk(&b, 0);
    let decode = median_us(|| b.decode.decode(0, black_box(bytes), black_box(&mut scratch)));
    let span = f64::from(m.time_per_div()) * f64::from(HDIV);
    let first = f64::from(m.delay) - span / 2.0;
    let trigger = || crate::board::record::record_trigger(black_box(&frame), &m, crate::sparse::POINTS, span, first);
    let search = median_us(|| {
        black_box(trigger());
    });
    let (ti, triggered) = trigger();
    assert!(triggered);
    println!("decode {decode:.3} us/record, legacy trigger {search:.3} us/record");
    for (factor, tdiv) in [(0, None), (10, Some(1e-6)), (200, Some(50e-9)), (2000, Some(5e-9))] {
        let v = view(m.clone(), tdiv);
        let mut p = prepared();
        let placement = median_us(|| {
            super::super::place(&b, &v, black_box(&mut p), &mut scratch);
            black_box(&p.plan);
        });
        let overview = reference_resampling(&frame, m.time_per_div(), ti);
        let detail = tdiv.map(|t| reference_resampling(&frame, t, ti));
        println!("{factor:4}x placement {placement:.3} us/record, legacy overview {overview:.3}, detail {detail:?}");
    }
}

fn reference_resampling(frame: &Frame, tdiv: f32, ti: f64) -> f64 {
    let n = crate::sparse::POINTS;
    let span = f64::from(tdiv) * f64::from(HDIV);
    let dt = span / n as f64;
    let mut channels: [Vec<f32>; NCH] = Default::default();
    median_us(|| {
        for (values, out) in frame.ch.iter().zip(&mut channels) {
            crate::board::record::resample_channel(
                black_box(values),
                (frame.dt, frame.envelope),
                AcqMode::Normal,
                (n, dt, -span / 2.0),
                ti,
                black_box(out),
            );
        }
    })
}

#[test]
fn measured_zoom_records_stay_on_codes_and_fold_the_decoded_values() {
    let mut m = model(4);
    m.meas = vec![(Meas::Vpp, WaveId::Ch(0)), (Meas::Vavg, WaveId::Ch(1)), (Meas::Vrms, WaveId::Ch(3))];
    let (b, _) = batch(&m);
    let v = view(m.clone(), Some(5e-9));
    let mut coded = prepared();
    coded.folds = crate::framestats::folds_for(&m);
    assert!(!coded.folds.is_empty());
    place(&b, &v, &mut coded, &mut Frame::default(), chunk(&b, 0));
    assert!(coded.zoom.as_ref().is_some_and(|z| z.plan.is_some()), "zoom placed");

    let mut decoded = Frame::default();
    b.decode.decode(0, chunk(&b, 0), &mut decoded);
    b.header.tag(&mut decoded);
    let mut expected = crate::framestats::folds_for(&m);
    let measured = fold_decoded(&decoded, &m, &mut expected);
    assert_eq!(coded.measured, measured);
    for ((key, got), (_, want)) in coded.folds.iter().zip(&expected) {
        assert_eq!(got, want, "{key:?}");
    }
}

mod fixtures;
mod pixels;
use pixels::reference_picture;
mod tiles_bench;
