// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::board::scripted::{self, Batch as _};
use crate::system::test_allocator::own_high_water;

#[test]
fn batch_allocator_high_water_scales_with_enabled_channels_and_sample_geometry() {
    let _scope = test_scope::Scope::new();
    let (peak, ()) = own_high_water(|| {
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
    // Run the production placement and tile jobs on the measuring thread.
    let pipe = Pipe::start(0, 0, counters());
    let (w, h) = (970, 448);
    pipe.set_view(Some(m), w, h, crate::sparse::Interp::Exact);
    let view = oos_hal::sync::lock(&pipe.view).view.clone().unwrap();
    let mut pool = Pool::new(1, 0, run);
    let mut arena = Arc::default();
    let mut jobs = Vec::new();
    let mut ph = Phosphor::new(w, h);
    let mut picture = || {
        let batch =
            pipe.feed_batch(bytes.take().unwrap(), fb, 0..frames, (m.hw_stamp, None), decode.clone(), codes.clone());
        let mut deposit = prepare(None, &pipe.slot, &batch, &view, None).unwrap();
        draw(&batch, &view, &mut deposit, &mut pool, &mut arena, &mut jobs);
        assert!(pipe.slot.put(deposit).is_none());
        drop(batch);
        bytes = Some(host::finish_batch(&pipe, 0).unwrap());
        let got = pipe.accept(&mut ph, &mut true, &mut [0; 4], &mut (0.0, 0.0));
        assert_eq!(got.frames, frames as u64);
    };
    let (peak, ()) = own_high_water(|| {
        for _ in 0..4 {
            picture();
        }
    });
    let (warm_peak, ()) = own_high_water(|| {
        for _ in 0..4 {
            picture();
        }
    });
    // One difference plane and fold row per enabled channel, decode scratch, and one screen record per placement.
    let placement = frames * channels * crate::sparse::POINTS * size_of::<f32>();
    let budget = channels * w * (h + 2) * 2 + fb * 2 + placement + 262_144;
    println!("BATCH_HEAP channels={channels} {plan} peak={peak} warm_peak={warm_peak} budget={budget}");
    assert!(peak <= budget, "{channels} channels: {peak} exceeds {budget}");
    assert!(warm_peak <= 32_768, "transient scratch must follow current sample geometry");
}
