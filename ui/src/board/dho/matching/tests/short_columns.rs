// SPDX-License-Identifier: GPL-2.0-or-later
//! Real matched decoder and worker, calibrated 4CH 3520-point native record geometry.
use super::*;
use crate::{
    drawpipe,
    model::{tscales, Model},
    sparse::Interp,
    wave::Phosphor,
};
fn geometry() -> (Settings, hz::Params, Model) {
    let (mut s, _) = setup(15, false, 3520);
    let mut p = hz::prep(15, 312_500_000, 3520, 0);
    p.memsel = 3520;
    let mut m = Model {
        tscale: tscales().iter().position(|&t| (t as f64 - 1e-6).abs() < 1e-9).unwrap(),
        trig_src: 1,
        ..Model::default()
    };
    for ch in &mut m.ch {
        ch.on = true;
    }
    s.frames = 142;
    assert_eq!(seq::frame_bytes(&s, &p), 28160);
    (s, p, m)
}
fn corpus(s: &Settings, p: &hz::Params, n: usize) -> Vec<u8> {
    (0..=n).flat_map(|i| record(s, p, i)).collect()
}
#[test]
fn corrected_dense_short_columns_keep_full_pixels_raw_newest_and_record_count() {
    let _scope = drawpipe::test_scope::Scope::new();
    let (s, p, original) = geometry();
    for mode in 0..3 {
        let mut m = original.clone();
        if mode == 1 {
            m.dots = crate::model::DotsMode::On;
        }
        if mode == 2 {
            m.tscale = tscales().iter().position(|&t| (t as f64 - 1e-7).abs() < 1e-10).unwrap();
        }
        for n in [127, 141] {
            let bytes = corpus(&s, &p, n);
            let fb = 28160;
            let (want, expected_tap) = reference_picture(&m, &bytes, fb, &s, &p, mode == 2);
            let counters = Box::leak(Box::new(drawpipe::Counters::default()));
            let pipe = drawpipe::Pipe::start(4, 0, counters);
            pipe.publish_view(&m, 1024, 480, Interp::Exact, Default::default());
            let mut cores = seed(&bytes[fb..2 * fb], &s, &p);
            let newest = Frame { seq: n as u64, ..Frame::default() };
            feed(
                &pipe,
                (Box::new(Bytes(bytes)), fb, 1..n + 1),
                (Default::default(), std::time::Instant::now()),
                (s.clone(), p, HeaderProbe::Off),
                Some(&mut cores),
                (Some(newest), None, true),
            );
            finish(&pipe);
            let mut got = Phosphor::new(1024, 480);
            let accepted = pipe.accept(&mut got, &mut false, &mut [0; 4], &mut (0.0, 0.0));
            assert_eq!(accepted.frames, n as u64);
            assert!(got.maps() == want.maps(), "mode {mode}, records {n}");
            let tap = pipe.take_waveform((1024, 480)).unwrap();
            for (actual, expected) in tap.ch.iter().zip(expected_tap.ch) {
                assert!(actual.iter().map(|v| v.to_bits()).eq(expected.iter().map(|v| v.to_bits())));
            }
            let selected = counters.routes.count(drawpipe::route::Route::Dense);
            assert_eq!(selected, if mode == 0 { n as u64 } else { 0 }, "actual route mode {mode}");
        }
    }
}

fn reference_picture(
    m: &Model,
    bytes: &[u8],
    fb: usize,
    s: &Settings,
    p: &hz::Params,
    short: bool,
) -> (Phosphor, Frame) {
    let oracle = seed(&bytes[fb..2 * fb], s, p);
    let mut want = Phosphor::new(1024, 480);
    let mut newest = Frame::default();
    let codes = short.then(|| {
        let mut cores = seed(&bytes[fb..2 * fb], s, p);
        let matched = BatchMatch::prepare(bytes, fb, 1..bytes.len() / fb, s, p, &mut cores);
        BatchMatch::dispatch(Some(matched), s.clone(), *p, (HeaderProbe::Off, None)).1.unwrap()
    });
    for (i, raw) in bytes.chunks_exact(fb).enumerate().skip(1) {
        newest = frozen(raw, s, p, &oracle);
        if let Some(codes) = &codes {
            let layout = codes(i, raw, &mut Frame::default()).unwrap();
            drawpipe::stage::fixture_bench::add_short_columns(&mut want, m, &newest, raw, layout);
        } else {
            drawpipe::draw_frame(&mut want, m, &newest, Interp::Exact, None);
        }
    }
    (want, newest)
}
#[test]
#[ignore = "bounded matched-worker comparison, OOS_PERF_TESTS=1 and one test thread"]
fn measured_matched_4ch_whole_worker() {
    assert_eq!(std::env::var("OOS_PERF_TESTS").as_deref(), Ok("1"));
    let _scope = drawpipe::test_scope::Scope::new();
    let (s, p, m) = geometry();
    for n in [127, 141] {
        let bytes = corpus(&s, &p, n);
        let pipe = drawpipe::Pipe::start(4, 0, Box::leak(Box::default()));
        pipe.publish_view(&m, 1024, 480, Interp::Exact, Default::default());
        let mut cores = Cores::default();
        for turn in 0..16 {
            let raw = Box::new(Bytes(bytes.clone()));
            let newest = Frame { seq: turn, ..Frame::default() };
            let mut ph = Phosphor::new(1024, 480);
            let start = std::time::Instant::now();
            feed(
                &pipe,
                (raw, 28160, 1..n + 1),
                (Default::default(), std::time::Instant::now()),
                (s.clone(), p, HeaderProbe::Off),
                Some(&mut cores),
                (Some(newest), None, true),
            );
            finish(&pipe);
            let elapsed = start.elapsed().as_nanos();
            assert_eq!(pipe.accept(&mut ph, &mut false, &mut [0; 4], &mut (0.0, 0.0)).frames, n as u64);
            if turn >= 4 {
                println!("MATCHED_WHOLE_WORKER selected={n} raw={} points=3520 width=1024 height=480 workers=4 matching=true route=source_bound turn={turn} feed_join_ns={elapsed} worker_matching=true",n+1);
            }
        }
    }
}

#[test]
fn each_lane_layout_preserves_pixels_and_tap_voltage_bits() {
    for (mask, len, rate) in [
        (1, 14064, 1_250_000_000),
        (3, 7032, 625_000_000),
        (10, 7032, 625_000_000),
        (13, 3520, 312_500_000),
        (15, 3520, 312_500_000),
    ] {
        for cores in [false, true] {
            for dots in [false, true] {
                check_lane_layout(mask, len, rate, (false, 1), cores, dots);
                check_lane_layout(mask, len, rate, (true, 1), cores, dots);
            }
        }
        check_lane_layout(mask, len, rate, (true, 3), false, false);
    }
}

fn check_lane_layout(mask: u32, len: u32, rate: u64, (header, compress): (bool, u32), cores: bool, dots: bool) {
    let (mut s, mut p) = setup(mask, header, len);
    s.compress = compress;
    s.window = None;
    p.rate = rate;
    let (_, _, mut model) = geometry();
    model.trig_src = mask.trailing_zeros() as usize;
    model.dots = if dots { crate::model::DotsMode::On } else { crate::model::DotsMode::Off };
    for (channel, ch) in model.ch.iter_mut().enumerate() {
        ch.on = mask & (1 << channel) != 0;
    }
    let frames = 5;
    let fb = seq::frame_bytes(&s, &p);
    let mut payload = p;
    payload.memsel = ((fb - if header { 16 } else { 0 }) / (2 * p.ratio as usize)) as u32;
    let bytes = corpus(&s, &payload, frames - 1);
    assert_eq!(bytes.len(), fb * frames);
    let counters = Box::leak(Box::new(drawpipe::Counters::default()));
    let pipe = drawpipe::Pipe::start(2, 0, counters);
    let (w, h) = (384, 112);
    pipe.publish_view(&model, w, h, Interp::Exact, Default::default());
    let mut want = Phosphor::new(w, h);
    let oracle = seed(&bytes[..fb], &s, &p);
    let mut decoded = Frame::default();
    for raw in bytes.chunks_exact(fb) {
        decoded = if cores {
            frozen(raw, &s, &p, &oracle)
        } else {
            super::super::super::hw::decode_probed(raw, &s, &p, 0, HeaderProbe::Off)
        };
        drawpipe::draw_frame(&mut want, &model, &decoded, Interp::Exact, None);
    }
    let mut matcher = seed(&bytes[..fb], &s, &p);
    let mut newest = if cores { Frame::default() } else { decoded.clone() };
    newest.seq = frames as u64;
    feed(
        &pipe,
        (Box::new(Bytes(bytes)), fb, 0..frames),
        (Default::default(), std::time::Instant::now()),
        (s, p, HeaderProbe::Off),
        cores.then_some(&mut matcher),
        (Some(newest), None, cores),
    );
    finish(&pipe);
    let mut got = Phosphor::new(w, h);
    assert_eq!(pipe.accept(&mut got, &mut false, &mut [0; 4], &mut (0.0, 0.0)).frames, frames as u64);
    assert_eq!(got.maps(), want.maps(), "mask {mask:04b} header={header} k={compress} cores={cores} dots={dots}");
    let tap = pipe.take_waveform((w, h)).unwrap();
    for (actual, expected) in tap.ch.iter().zip(decoded.ch) {
        assert_eq!(actual.len(), expected.len());
        assert!(actual.iter().zip(expected).all(|(a, b)| a.to_bits() == b.to_bits()));
    }
    let route = if compress > 1 {
        drawpipe::route::Route::Pairs
    } else if dots {
        drawpipe::route::Route::Dots
    } else {
        drawpipe::route::Route::Dense
    };
    assert_eq!(counters.routes.count(route), frames as u64);
}
