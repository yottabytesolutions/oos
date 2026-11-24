// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::board::scripted::{self, Batch as _};
use crate::drawpipe::{host, Counters, Pipe};
use crate::model::{DotsMode, Model};
use crate::sparse::Interp;
use crate::wave::Phosphor;

fn model(mask: u32) -> Model {
    let mut model = Model { dots: DotsMode::Off, ..Default::default() };
    model.tscale = crate::model::tscales().iter().position(|&v| v == 1e-6).unwrap();
    model.trig_src = mask.trailing_zeros() as usize;
    for (c, channel) in model.ch.iter_mut().enumerate() {
        channel.on = mask & (1 << c) != 0;
    }
    model
}

#[test]
fn dense_normal_lines_choose_codes_for_every_mask() {
    for mask in 1..16 {
        let m = model(mask);
        for space in [0.025, 0.25, 0.5, 0.75] {
            let width = 1024;
            let window = m.time_per_div() as f64 * HDIV as f64;
            let plain = Frame { dt: space * window / width as f64, ..Default::default() };
            for dots in [false, true] {
                let proj = Projection { dots, ..standard::projection(&m, width, 480, space) };
                for mode in [AcqMode::Normal, AcqMode::Peak, AcqMode::HiRes, AcqMode::Average] {
                    let expected = match (mode, dots) {
                        (AcqMode::HiRes, _) if space == 0.025 => Route::Boxcar,
                        (AcqMode::HiRes, _) => Route::Upsampled,
                        _ if space > 0.5 => Route::Upsampled,
                        (_, true) => Route::Dots,
                        (AcqMode::Peak, false) => Route::Peak,
                        _ => Route::Dense,
                    };
                    assert_eq!(route(&plain, &proj, mode, true), expected, "{mask:04b} {mode:?} {space}");
                }
            }
        }
    }
    let proj = standard::projection(&model(1), 1024, 480, 0.25);
    assert_eq!(route(&Frame::default(), &proj, AcqMode::Normal, false), Route::Volts);
    for frame in [Frame { roll: true, ..Frame::default() }, Frame { scan: true, ..Frame::default() }] {
        assert_eq!(route(&frame, &proj, AcqMode::Normal, true), Route::Record);
    }
    for depth in [100_000, crate::compress::MAX_DEPTH as usize + 1] {
        let frame = Frame { envelope: true, acq: (1.25e9, depth), ..Frame::default() };
        for codes in [false, true] {
            let route = route(&frame, &proj, AcqMode::Normal, codes);
            assert_eq!(route, Route::Pairs);
            assert_eq!(route.screen_raster(false, &frame), depth > crate::compress::MAX_DEPTH as usize);
        }
    }
}

#[test]
fn every_scripted_mask_uses_dense_pool_placement_including_newest() {
    for mask in 1..16 {
        let m = model(mask);
        let counters = Box::leak(Box::new(Counters::default()));
        let pipe = Pipe::start(2, 0, counters);
        pipe.publish_view(&m, 384, 112, Interp::Exact, Default::default());
        let (input, frames, _) = scripted::draw_route_batch(&m, &Default::default(), 3);
        let (fb, decode, codes) = (input.frame_bytes(), input.decode(), input.codes());
        let newest = Arc::new(input.newest(0));
        pipe.feed_with(
            Box::new(input.into_bytes()),
            fb,
            0..frames,
            Default::default(),
            decode,
            codes,
            Some(newest.clone()),
        );
        host::finish_batch(&pipe, 0).unwrap();
        let mut ph = Phosphor::new(384, 112);
        assert_eq!(pipe.accept(&mut ph, &mut false, &mut [0; 4], &mut (0.0, 0.0)).frames, frames as u64);
        assert_eq!(counters.routes.total(), frames as u64);
        assert_eq!(counters.routes.count(Route::Dense), frames as u64, "{mask:04b}");
        assert!(Arc::ptr_eq(&pipe.take_waveform((384, 112)).unwrap(), &newest));
    }
}
