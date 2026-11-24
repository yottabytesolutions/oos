// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

pub(super) fn reference_channels(
    frame: &Frame,
    (n, window, first): (usize, f64, f64),
    ti: f64,
    out: &mut [Vec<f32>; NCH],
) {
    let dt = window / n as f64;
    for (samples, output) in frame.ch.iter().zip(out) {
        output.clear();
        if samples.len() < 2 {
            continue;
        }
        if frame.dt > 2.0 * dt {
            oos_dsp::interp::lanczos8_grid(samples, ti + first / frame.dt, dt / frame.dt, n, output);
            continue;
        }
        let last = samples.len() - 1;
        output.extend((0..n).map(|k| {
            let x = ti + (first + k as f64 * dt) / frame.dt;
            if !(0.0..=last as f64).contains(&x) {
                return f32::NAN;
            }
            let i = (x.floor() as usize).min(last - 1);
            let (a, b) = (f64::from(samples[i]), f64::from(samples[i + 1]));
            (a + (b - a) * (x - i as f64)) as f32
        }));
    }
}

pub(super) fn compare_fold(
    batch: &Arc<Batch>,
    view: &View,
    model: &Model,
    range: Range<usize>,
    count: u32,
    scales: [f32; NCH],
) -> f32 {
    let m = Model { avg_count: count, ..model.clone() };
    let view = Arc::new(View { gen: view.gen, m, interp: view.interp, w: view.w, h: view.h, zoom: view.zoom });
    let mut expected = None;
    for index in range {
        let frame = decoded_newest_at(batch, index).map_or_else(
            || {
                let mut frame = Frame::default();
                batch.decode.decode(index, chunk(batch, index), &mut frame);
                frame
            },
            Frame::clone,
        );
        let record = hw_record(&view.m, Arc::new(frame));
        assert!(!record.pairs);
        expected = Some(original(expected.as_ref(), record, count));
    }
    let (output, _) = average_batch(&mut Painter::new(4, 0), batch, &view);
    let output = output.unwrap();
    let mut actual = Default::default();
    output.apply(None, &mut actual);
    output.shown();
    adc_error(&actual, &expected.unwrap().ch, scales)
}

#[test]
fn dense_dma_point_ema_matches_fractionally_aligned_volts() {
    for (tdiv, (samples, dt)) in [(1e-6, (13_000, 0.8e-9)), (10e-6, (126_000, 0.8e-9))] {
        for lanes in 1..=4 {
            let pipe = Pipe::start(0, 0, Box::leak(Box::default()));
            // The shifted grid extends past the record on one side. Compare its NaN mask too.
            let m = Model { acq: AcqMode::Average, delay: (tdiv / 4.0) as f32, ..model(tdiv, true, false) };
            pipe.publish_view(&m, 128, 64, Interp::Exact, Default::default());
            let view = oos_hal::sync::lock(&pipe.view).view.clone().unwrap();
            let batch = raw_fixture(&pipe, lanes, 9, (samples, dt));
            let (n, window, first) = hw_grid(&m);
            assert!(window / n as f64 / dt >= 2.0);
            let mut frame = Frame::default();
            let mut edge = |index| {
                batch.decode.decode(index, chunk(&batch, index), &mut frame);
                crate::board::record::record_trigger(&frame, &m, n, window, first)
            };
            let (start, found) = edge(0);
            let (end, found_end) = edge(8);
            assert!(found && found_end && (start - end).abs() > 0.1, "{start} {end}");
            assert!(start.fract() != 0.0 || end.fract() != 0.0);
            verify_codes(&format!("dense {tdiv} s/div, {lanes} lanes"), &batch, &view, 0..9);
        }
    }
}
