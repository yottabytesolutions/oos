// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::drawpipe::tests::{model, Bytes};
use crate::system::test_allocator::count_own;
use crate::{
    board::record::hw_grid,
    wave::{hw_record, Record},
};

const VOLTS_PER_COUNT: f32 = 1.0 / 7500.0;
const ADC_TOLERANCE: f32 = 0.01;

fn synthetic(frame: &mut Frame, index: usize) {
    frame.dt = 2.0e-9;
    frame.pre = 270;
    for (channel, samples) in frame.ch[..2].iter_mut().enumerate() {
        samples.clear();
        if channel == 1 && index % 13 == 1 {
            continue;
        }
        let fraction = (index % 17) as f64 / 17.0;
        samples.extend((0..540).map(|i| {
            if channel == 1 && ((12..21).contains(&i) && index.is_multiple_of(7) || i > 520 && index % 8 == 3) {
                f32::NAN
            } else {
                let t = i as f64 - 270.0 - fraction;
                let signal = 5300.0 * (t * (0.15 + 0.05 * channel as f64)).sin();
                let noise = ((i * 19 + index * 31 + channel * 43) % 59) as f64 - 29.0;
                (signal + noise).round() as f32 * VOLTS_PER_COUNT
            }
        }));
    }
}

fn fixture(pipe: &Pipe, first: usize, len: usize) -> Arc<Batch> {
    let decode: DecodeInto = Arc::new(|bytes, frame| synthetic(frame, usize::from(bytes[0])));
    let id = Some(Default::default());
    let bytes = (first..first + len).map(|i| i as u8).collect();
    let mut batch = pipe.feed_batch(Box::new(Bytes(bytes)), 1, 0..len, (0, id), decode, None);
    let mut newest = Frame { seq: (first + len) as u64, ..Default::default() };
    synthetic(&mut newest, usize::from((first + len - 1) as u8));
    Arc::get_mut(&mut batch).unwrap().newest = Some(Arc::new(newest));
    batch
}

fn raw_fixture(pipe: &Pipe, lanes: usize, len: usize, (samples, dt): (usize, f64)) -> Arc<Batch> {
    let stride = lanes.next_power_of_two();
    let mut layout = CodeLayout::uncorrected(
        stride,
        std::array::from_fn(|c| (c < lanes).then_some(c)),
        [(VOLTS_PER_COUNT, 0.0); NCH],
        |word, (scale, offset)| f32::from((word ^ 0x8000) as i16) * scale - offset,
    );
    let corrections = std::array::from_fn(|c| crate::fastdraw::CorrectedCodes {
        offsets: [25, -13, 4, 5, -30, 10, 4, -5],
        scale: 1.0 + c as f64,
        offset: c as f64 / 4.0,
    });
    layout.corrected = Some(corrections);
    let read: CodesInto = Arc::new(move |_, _, f| {
        f.dt = dt;
        f.pre = samples / 2;
        f.ch.iter_mut().for_each(Vec::clear);
        Some(layout)
    });
    let decode: DecodeInto = Arc::new(move |bytes, f| {
        f.dt = dt;
        f.pre = samples / 2;
        for (c, (out, cal)) in f.ch.iter_mut().zip(corrections).enumerate() {
            out.clear();
            if c < lanes {
                out.extend(
                    bytes
                        .as_chunks::<2>()
                        .0
                        .iter()
                        .skip(c)
                        .step_by(stride)
                        .enumerate()
                        .map(|(i, &word)| cal.volts(cal.code(i, u16::from_le_bytes(word)))),
                );
            }
        }
    });
    let frame_bytes = samples * stride * 2;
    let mut bytes = Vec::with_capacity(frame_bytes * len);
    for record in 0..len {
        for i in 0..samples {
            for (c, cal) in corrections.iter().enumerate().take(stride) {
                let x = (i as f64 - (samples / 2) as f64 - record as f64 / len as f64) * (0.14 + 0.04 * c as f64);
                let word = 32768 + (5300.0 * x.sin()) as i32 + cal.offsets[i % 8];
                bytes.extend((word as u16).to_le_bytes());
            }
        }
    }
    let mut newest = Frame::default();
    decode(&bytes[(len - 1) * frame_bytes..], &mut newest);
    let mut batch = pipe.feed_batch(Box::new(Bytes(bytes)), frame_bytes, 0..len, (0, None), decode, Some(read));
    Arc::get_mut(&mut batch).unwrap().newest = Some(Arc::new(newest));
    batch
}

#[test]
fn dma_grid_matches_the_volts_oracle_with_one_to_four_lanes() {
    for (tdiv, delay) in [(5e-9, 0.0), (50e-9, -100e-9), (500e-9, 0.0), (1e-6, 150e-9)] {
        for lanes in 1..=4 {
            let pipe = Pipe::start(0, 0, Box::leak(Box::default()));
            let m = Model { delay, ..model(tdiv, true, false) };
            pipe.publish_view(&m, 128, 64, Interp::Exact, Default::default());
            let view = oos_hal::sync::lock(&pipe.view).view.clone().unwrap();
            let batch = raw_fixture(&pipe, lanes, 17, (256, 2e-9));
            verify_codes(&format!("{tdiv} s/div, {lanes} lanes"), &batch, &view, 0..17);
        }
    }
}

fn original(previous: Option<&Record>, mut record: Record, n: u32) -> Record {
    if let Some(previous) = previous {
        let a = 1.0 / n as f32;
        for c in 0..NCH {
            if record.ch[c].len() == previous.ch[c].len() {
                for (x, p) in record.ch[c].iter_mut().zip(&previous.ch[c]) {
                    if p.is_finite() {
                        *x = p + (*x - p) * a;
                    }
                }
            }
        }
    }
    record
}

pub(in crate::drawpipe::stage) fn verify_codes(name: &str, batch: &Arc<Batch>, view: &View, range: Range<usize>) {
    let mut scratch = Scratch::default();
    let mut decoded = Frame::default();
    let mut meta = Frame::default();
    let mut reference = Default::default();
    let model = Model { acq: AcqMode::Average, avg_hw: false, ..view.m.clone() };
    let grid = hw_grid(&model);
    let screen_dt = grid.1 / grid.0 as f64;
    let mut stats = [0.0f32; 2];
    let mut scales = [0.0; NCH];
    for index in range.clone() {
        let bytes = chunk(batch, index);
        let layout = batch.codes.as_ref().unwrap()(index, bytes, &mut meta).unwrap();
        scales = std::array::from_fn(|c| {
            let codes = layout.channel(bytes, c);
            codes.corrected.map_or(codes.affine.0, |cal| (cal.scale / 7500.0) as f32).abs()
        });
        let record = decoded_newest_at(batch, index).unwrap_or_else(|| {
            batch.decode.decode(index, bytes, &mut decoded);
            &decoded
        });
        let (want, found) = crate::board::record::record_trigger(record, &model, grid.0, grid.1, grid.2);
        let (_, _, got, triggered) = codes::read(&mut scratch, batch, &model, grid, screen_dt, index)
            .unwrap_or_else(|cause| panic!("{name} record {index} used fallback bit {}", cause.bit()));
        assert_eq!((got.to_bits(), triggered), (want.to_bits(), found), "{name} record {index}");
        dense::reference_channels(record, grid, want, &mut reference);
        stats[0] = stats[0].max(adc_error(&scratch.screen, &reference, scales));
    }
    for count in [2, 4, 16, 1024] {
        stats[1] = stats[1].max(dense::compare_fold(batch, view, &model, range.clone(), count, scales));
    }
    // The Q14 coefficients introduce a small additional Lanczos error. The linear fold
    // also allows several f32 roundings from joining and reseeding around a nonzero offset.
    let tolerance = if meta.dt > 2.0 * screen_dt { [8.0, 8.0] } else { [0.125, 0.5] };
    println!("{name}: {} records, trigger error 0 samples, grid/fold ADC errors {stats:?}", range.len());
    assert!(stats.into_iter().zip(tolerance).all(|(error, limit)| error <= limit), "{name}: limits {tolerance:?}");
}

fn adc_error(actual: &[Vec<f32>; NCH], expected: &[Vec<f32>; NCH], scales: [f32; NCH]) -> f32 {
    let mut error = 0.0f32;
    for ((actual, expected), scale) in actual.iter().zip(expected).zip(scales) {
        assert_eq!(actual.len(), expected.len());
        for (&y, &x) in actual.iter().zip(expected) {
            assert_eq!(y.is_nan(), x.is_nan());
            if x.is_finite() {
                error = error.max((y - x).abs() / scale);
            }
        }
    }
    error
}

#[test]
fn worker_average_matches_ordered_record_ema_to_one_hundredth_adc_count() {
    for count in [2, 4, 16, 1024] {
        let pipe = Pipe::start(0, 0, Box::leak(Box::default()));
        let m = Model { acq: AcqMode::Average, avg_count: count, ..model(100e-9, true, false) };
        pipe.publish_view(&m, 128, 64, Interp::Exact, Default::default());
        let view = oos_hal::sync::lock(&pipe.view).view.clone().unwrap();
        let (points, window, first) = hw_grid(&m);
        let edge = |i| {
            let mut f = Frame::default();
            synthetic(&mut f, i);
            crate::board::trigger_point(&f, &m, points, window, first).unwrap()
        };
        assert!((edge(0) - edge(8)).abs() > 0.1);
        for workers in 1..=4 {
            let mut painter = Painter::new(workers, 0);
            let mut expected = None;
            let mut actual = None;
            let mut start = 0;
            for (batch, len) in [1, 2, 48, 80, 1, 2, 48, 80].into_iter().enumerate() {
                if batch == 4 {
                    (actual, expected) = (None, None);
                }
                let b = fixture(&pipe, start, len);
                for index in start..start + len {
                    let mut raw = Arc::<Frame>::default();
                    synthetic(Arc::get_mut(&mut raw).unwrap(), usize::from(index as u8));
                    expected = Some(original(expected.as_ref(), hw_record(&m, raw), count));
                }
                let (output, _) = average_batch(&mut painter, &b, &view);
                let output = output.unwrap();
                let raw = newest(&b).unwrap().clone();
                let mut rec = output.record(&raw);
                output.apply(actual.as_ref(), &mut rec.ch);
                assert_eq!(output.shown().0, len);
                let expected = expected.as_ref().unwrap();
                assert_eq!((rec.t0, rec.dt, rec.triggered), (expected.t0, expected.dt, expected.triggered));
                let retained = rec.raw.as_ref().unwrap();
                let oracle = expected.raw.as_ref().unwrap();
                assert!(Arc::ptr_eq(&retained.frame, &raw));
                assert_eq!((retained.i0, retained.i1, retained.t0), (oracle.i0, oracle.i1, oracle.t0));
                for (ours, oracle) in rec.ch.iter().zip(&expected.ch) {
                    assert_eq!(ours.len(), oracle.len());
                    for (&y, &x) in ours.iter().zip(oracle) {
                        let adc_counts = (y - x).abs() / VOLTS_PER_COUNT;
                        assert!(
                            adc_counts <= ADC_TOLERANCE || (x.is_nan() && y.is_nan()),
                            "count={count} len={len} workers={workers} batch={batch} y={y} x={x} adc_counts={adc_counts}"
                        );
                    }
                }
                actual = Some(rec);
                start += len;
            }
        }
    }
}

fn counted_job(scratch: &mut Scratch, job: &mut Job) {
    const WARMUP: usize = 8;
    thread_local! { static JOBS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }
    let (allocations, ()) = count_own(|| run(scratch, job));
    if JOBS.get() >= WARMUP {
        assert_eq!(allocations, 0, "worker decode, refine, resample and accumulate");
    }
    JOBS.set(JOBS.get() + 1);
}

#[test]
fn warmed_average_reuses_worker_and_output_buffers() {
    let pipe = Pipe::start(0, 0, Box::leak(Box::default()));
    let m = Model { acq: AcqMode::Average, avg_count: 16, ..model(1e-6, true, false) };
    pipe.publish_view(&m, 128, 64, Interp::Exact, Default::default());
    let view = oos_hal::sync::lock(&pipe.view).view.clone().unwrap();
    let b = raw_fixture(&pipe, 4, 48, (8_192, 1.25e-9));
    let mut painter = Painter::new(4, 0);
    painter.pool = Pool::new(4, 0, counted_job);
    let mut screen = Default::default();
    let mut pass = || {
        let (output, _) = average_batch(&mut painter, &b, &view);
        let output = output.unwrap();
        output.apply(None, &mut screen);
        assert_eq!(output.shown().0, 48);
    };
    for _ in 0..8 {
        pass();
    }
    assert_eq!(count_own(pass).0, 0, "coordinator and UI screen transform");
}

#[test]
fn average_folds_the_dma_decode_when_the_newest_tap_is_a_separate_replay() {
    let pipe = Pipe::start(0, 0, Box::leak(Box::default()));
    let m = Model { acq: AcqMode::Average, avg_count: 2, ..model(100e-9, false, false) };
    pipe.publish_view(&m, 128, 64, Interp::Exact, Default::default());
    let view = oos_hal::sync::lock(&pipe.view).view.clone().unwrap();
    let decode: DecodeInto = Arc::new(|_, frame| {
        frame.dt = 2e-9;
        frame.pre = 280;
        frame.ch[0].resize(560, 0.25);
    });
    let codes: CodesInto = Arc::new(|_, _, _| None);
    let mut batch = pipe.feed_batch(Box::new(Bytes(vec![0])), 1, 0..1, (0, None), decode, Some(codes));
    let replay = Arc::new(Frame {
        dt: 1e-9,
        pre: 560,
        ch: [vec![0.75; 1120], Vec::new(), Vec::new(), Vec::new()],
        ..Default::default()
    });
    Arc::get_mut(&mut batch).unwrap().newest = Some(replay.clone());

    assert!(decoded_newest_at(&batch, 0).is_none());
    let (output, _) = average_batch(&mut Painter::new(1, 0), &batch, &view);
    let output = output.unwrap();
    let mut record = output.record(&replay);
    output.apply(None, &mut record.ch);
    assert_eq!(record.ch[0][1800], 0.25);
    assert_eq!(batch.counters.routes.fallbacks.entries().find(|(name, _)| *name == "no_words").unwrap().1, 1);
    let raw = record.raw.as_ref().unwrap();
    assert!(Arc::ptr_eq(&raw.frame, &replay));
    assert_eq!((raw.i0, raw.i1, raw.t0), crate::board::record::raw_window(560.0, record.t0, hw_grid(&m).1, 1e-9, 1120));
}

#[test]
fn average_claim_on_a_pairs_grid_is_a_settings_loss() {
    let counters = Box::leak(Box::<Counters>::default());
    let pipe = Pipe::start(0, 0, counters);
    let id = oos_core::change::Identity::default();
    let m = Model { acq: AcqMode::Average, ..model(2e-6, true, false) };
    let mut batch = fixture(&pipe, 0, 2);
    let batch_mut = Arc::get_mut(&mut batch).unwrap();
    Arc::get_mut(batch_mut.newest.as_mut().unwrap()).unwrap().envelope = true;
    batch_mut.decode = crate::drawpipe::decoder::Decoder::Plain(Arc::new(|bytes, frame| {
        synthetic(frame, usize::from(bytes[0]));
        frame.envelope = true;
    }));
    assert!(average::input_ok(&pipe, id, true));
    pipe.publish_view(&m, 128, 64, Interp::Exact, id);
    assert!(!average::input_ok(&pipe, id, true));
    let view = batch_view(&batch, &oos_hal::sync::lock(&pipe.view), pipe.slot.reset_before()).unwrap();
    Counters::add(&counters.acquired, 2);

    publish_average(&mut Painter::new(1, 0), &pipe.slot, &batch, &view, batch.clock);
    drop(batch);

    assert!(pipe.take_waveform((128, 64)).is_none());
    assert_eq!(counters.read(), (2, 0, 2));
    let text = counters.records_text();
    assert!(text.contains("records read=2 deposited=0 shown=0"), "{text}");
    assert!(text.contains("lost.settings=2 "), "{text}");
    assert!(text.contains("lost.error=0 "), "{text}");
}

mod dense;

#[test]
fn average_tap_exceeding_its_reservation_is_a_buffer_loss() {
    let counters = Box::leak(Box::<Counters>::default());
    let pipe = Pipe::start(0, 0, counters);
    let id = oos_core::change::Identity::default();
    let m = Model { acq: AcqMode::Average, ..model(100e-9, true, false) };
    pipe.publish_view(&m, 128, 64, Interp::Exact, id);
    let view = oos_hal::sync::lock(&pipe.view).view.clone().unwrap();
    let mut batch = fixture(&pipe, 0, 3);
    let raw_samples = newest(&batch).unwrap().ch.iter().map(Vec::capacity).sum();
    let charge = pipe.slot.reserve_tap(raw_samples, batch.view_gen_at_submit, Some(id)).unwrap();
    Arc::get_mut(&mut batch).unwrap().header.tap = Some(charge);
    Counters::add(&counters.acquired, 3);

    publish_average(&mut Painter::new(1, 0), &pipe.slot, &batch, &view, batch.clock);
    drop(batch);

    assert!(pipe.take_waveform((128, 64)).is_none());
    assert_eq!(counters.read(), (3, 0, 3));
    let text = counters.records_text();
    assert!(text.contains("records read=3 deposited=0 shown=0"), "{text}");
    assert!(text.contains("lost.settings=0 "), "{text}");
    assert!(text.contains("lost.buffer=3 "), "{text}");
}
