// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::drawpipe::{Helper, Pipe};
use crate::wave::Phosphor;

#[test]
fn batch_allocator_high_water_scales_with_enabled_channels_and_sample_geometry() {
    let _scope = Scope::new();
    let (peak, ()) = HEAP.own_high_water(|| {
        std::hint::black_box(vec![0u8; 32_768]);
    });
    assert_eq!(peak, 32_768, "high-water accounting must observe freed transients");
    for channels in [1, 2, 4] {
        for (tdiv, depth) in [(100e-9, 0), (10e-6, 0), (10e-6, 1), (200e-6, 0)] {
            let mut m = Model { dots: DotsMode::Off, mdepth: depth, ..Default::default() };
            m.meas.clear();
            m.meas2.clear();
            m.tscale = tscales().iter().position(|&t| (t - tdiv).abs() < tdiv * 1e-4).unwrap();
            for (c, ch) in m.ch.iter_mut().enumerate() {
                ch.on = c < channels;
            }
            measure_batch(&m, channels);
        }
    }
}

fn measure_batch(m: &Model, channels: usize) {
    let (input, frames, plan) = scripted::draw_route_batch(m, &Default::default(), 4);
    let (fb, decode, codes) = (input.frame_bytes(), input.decode(), input.codes());
    let mut bytes: Option<Box<dyn RawBatch>> = Some(Box::new(input.into_bytes()));
    // The production helper runs the same draw_range/fresh_acc as workers, synchronously here so harness
    // bookkeeping and worker scheduling cannot affect the calling thread's allocator high-water measurement.
    let pipe = Pipe::start(0, 0, drawpipe::counters());
    let (w, h) = (970, 448);
    pipe.set_view(Some(m), w, h, crate::sparse::Interp::Exact);
    let mut helper = Helper::default();
    let mut ph = Phosphor::new(w, h);
    let mut draw = || {
        let held = pipe.feed_codes(bytes.take().unwrap(), fb, frames, m.hw_stamp, decode.clone(), codes.clone());
        while pipe.help(&mut helper) {}
        drop(held);
        bytes = Some(drawpipe::host::finish_batch(&pipe, 0).unwrap());
        let got = pipe.merge(&mut ph, &mut true, &mut [0; 4], &mut (0.0, 0.0));
        assert_eq!(got.frames, frames as u64);
    };
    let (peak, ()) = HEAP.own_high_water(|| {
        for _ in 0..4 {
            draw();
        }
    });
    let (warm_peak, ()) = HEAP.own_high_water(|| {
        for _ in 0..4 {
            draw();
        }
    });
    let decoded = if channels <= 2 && fb >= drawpipe::CODES_MIN_BYTES { 0 } else { fb * 2 };
    // One retained plane per enabled channel, current decoded samples, and bounded first-use bookkeeping.
    let budget = channels * w * h * 2 + decoded + 262_144;
    println!("BATCH_HEAP channels={channels} {plan} peak={peak} warm_peak={warm_peak} budget={budget}");
    assert!(peak <= budget, "{channels} channels: {peak} exceeds {budget}");
    assert!(warm_peak <= 32_768, "transient scratch must follow current sample geometry");
}
