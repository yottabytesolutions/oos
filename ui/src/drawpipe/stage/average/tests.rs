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
    let m = Model { acq: AcqMode::Average, avg_count: 16, ..model(100e-9, true, false) };
    pipe.publish_view(&m, 128, 64, Interp::Exact, Default::default());
    let view = oos_hal::sync::lock(&pipe.view).view.clone().unwrap();
    let b = fixture(&pipe, 0, 48);
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
    let batch = fixture(&pipe, 0, 2);
    let dt = newest(&batch).unwrap().dt;
    assert!(average::geometry_ok(&pipe, id, dt, false));
    pipe.publish_view(&m, 128, 64, Interp::Exact, id);
    assert!(!average::geometry_ok(&pipe, id, dt, false));
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
