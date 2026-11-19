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
    let mut painter = Painter::new(1, 0);
    let mut ph = Phosphor::new(w, h);
    let mut picture = || {
        let batch =
            pipe.feed_batch(bytes.take().unwrap(), fb, 0..frames, (m.hw_stamp, None), decode.clone(), codes.clone());
        let deposit = painter.batch(None, &pipe.slot, &batch, &view, None).unwrap();
        assert_eq!(deposit.planes.iter().map(|b| b.plane.d.len()).sum::<usize>(), channels * w * (h + 1));
        assert!(deposit.planes.iter().all(|b| m.ch[b.channel].on));
        assert!(pipe.slot.put(deposit).is_ok());
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

#[test]
fn decoded_dots_use_one_scratch_frame_for_a_dense_slice() {
    use super::super::tests::{model, Bytes};

    let mut m = model(10e-6, false, true);
    m.meas = vec![(Meas::Vavg, WaveId::Ch(0))];
    let (w, h) = (970, 48);
    let pipe = Pipe::start(0, 0, Box::leak(Box::default()));
    pipe.set_view(Some(&m), w, h, Interp::Exact);
    let view = oos_hal::sync::lock(&pipe.view).view.clone().unwrap();
    let decode: IndexedDecodeInto = Arc::new(|index, _, frame| {
        frame.dt = 4e-9;
        frame.pre = 50_000;
        frame.ch[0].resize(100_000, 0.0);
        for (i, v) in frame.ch[0].iter_mut().enumerate() {
            *v = ((i + index) % 137) as f32 / 137.0 - 0.5;
        }
    });
    let batch = pipe.feed_batch(
        Box::new(Bytes(vec![0; SLICE])),
        1,
        0..SLICE,
        (0, None),
        decoder::Decoder::Prepared(decode.clone()),
        None,
    );
    let mut painter = Painter::new(1, 0);
    let (peak, deposit) = own_high_water(|| painter.batch(None, &pipe.slot, &batch, &view, None).unwrap());
    assert_eq!(deposit.records, SLICE as u64);
    let scratch = 100_000 * size_of::<f32>();
    let planes = w * (h + 2) * size_of::<u16>();
    assert!(peak <= scratch + planes + 262_144, "decoded dot placement peaked at {peak} bytes");

    let mut actual = Phosphor::new(w, h);
    assert!(pipe.slot.put(deposit).is_ok());
    pipe.accept(&mut actual, &mut true, &mut [0; 4], &mut (0.0, 0.0));
    let mut expected = Phosphor::new(w, h);
    let mut frame = Frame::default();
    for index in 0..SLICE {
        decode(index, &[], &mut frame);
        super::super::draw_frame(&mut expected, &m, &frame, Interp::Exact, None);
    }
    assert!(expected.map[0].iter().any(|&v| v > 0));
    assert_eq!(actual.map, expected.map);
}

#[test]
fn four_channel_plans_recheck_the_claimed_mode_and_identity() {
    let _scope = test_scope::Scope::new();
    for compress in [false, true] {
        for tdiv in [1e-6, 10e-6] {
            for columns in [271, 399, 970, 1920, 7680] {
                check_claimed_plan(4, compress, tdiv, columns, true);
            }
        }
    }
}

#[test]
fn unsupported_geometry_keeps_budget_recovery() {
    let _scope = test_scope::Scope::new();
    check_claimed_plan(2, false, 10e-6, 177, false);
}

fn check_claimed_plan(channels: usize, compress: bool, tdiv: f32, columns: usize, fits: bool) {
    use oos_core::change::{Identity, Request};
    let mut m = Model {
        tscale: tscales().iter().position(|&t| (t - tdiv).abs() < 1e-12).unwrap(),
        dots: DotsMode::Off,
        ..Default::default()
    };
    for (i, ch) in m.ch.iter_mut().enumerate() {
        ch.on = i < channels;
    }
    let sw = crate::sysconf::Switches { compress, ..Default::default() };
    let (input, frames, _) = scripted::draw_route_batch(&m, &sw, 0);
    let (fb, decode, codes) = (input.frame_bytes(), input.decode(), input.codes());
    let pipe = Pipe::start(0, 0, counters());
    let id = Identity::default();
    pipe.publish_view(&m, 970, 448, sw.interp, id);
    let mut batch =
        pipe.feed_batch(Box::new(input.into_bytes()), fb, 0..frames, (0, Some(id)), decode.clone(), codes.clone());
    let (raw, _, _) = scripted::draw_route_batch(&m, &Default::default(), 1);
    Arc::get_mut(&mut batch).unwrap().newest = Some(Arc::new(raw.newest(0)));
    // Mapping can change without changing acquisition identity. Claim the new projection.
    m.dots = if compress { DotsMode::Off } else { DotsMode::On };
    pipe.publish_view(&m, columns, 448, sw.interp, id);
    let view = batch_view(&batch, &oos_hal::sync::lock(&pipe.view), 0).unwrap();
    assert_eq!(view.w, columns);
    assert_eq!(view.m.dots, m.dots);
    let deposited = pipe.counters.deposited.load(Ordering::Relaxed);
    let mut painter = Painter::new(1, 0);
    if !fits {
        assert_eq!(frames, 14);
        assert_eq!(painter.plan(&batch, &view, (0..frames).step_by(SLICE)), (4970, CountFormat::Narrow));
    }
    let discarded = pipe.counters.read().2;
    let deposit = painter.batch(None, &pipe.slot, &batch, &view, Some(id));
    assert_eq!(deposit.is_some(), fits);
    if let Some(deposit) = deposit {
        assert_eq!(deposit.records, frames as u64);
        assert_eq!(deposit.format(), CountFormat::Narrow);
        assert!(deposit.extent.hits() <= HIT_LIMIT);
    }
    let tap = pipe.take_waveform((columns, 448)).unwrap();
    assert!(Arc::ptr_eq(&tap, batch.newest.as_ref().unwrap()));
    let eligibility = if fits { PictureEligibility::Pending } else { PictureEligibility::Drawable };
    assert_eq!(tap.eligibility(), eligibility);
    assert_eq!(pipe.counters.deposited.load(Ordering::Relaxed) - deposited, if fits { frames as u64 } else { 0 });
    assert_eq!(pipe.counters.read().2 - discarded, if fits { 0 } else { frames as u64 });
    assert_eq!(batch.next.load(Ordering::Relaxed), frames);
    let stale = pipe.feed_batch(
        Box::new(super::super::tests::Bytes(batch.lease.raw.as_ref().unwrap().bytes().to_vec())),
        fb,
        0..frames,
        (0, Some(id)),
        decode,
        codes,
    );
    let (_, cleared) = super::super::identity_tests::identities(Request::Clear);
    pipe.publish_view(&m, columns, 448, sw.interp, cleared);
    assert_eq!(pipe.read_limit(id, None), None);
    let discarded = pipe.counters.read().2;
    assert!(batch_view(&stale, &oos_hal::sync::lock(&pipe.view), 0).is_none());
    assert_eq!(pipe.counters.read().2 - discarded, frames as u64);
    assert_eq!(stale.next.load(Ordering::Relaxed), frames);
}

#[test]
fn admission_covers_sparse_remap_before_claim() {
    let _scope = test_scope::Scope::new();
    let mut m = Model {
        tscale: tscales().iter().position(|&t| (t - 10e-9).abs() < 1e-15).unwrap(),
        dots: DotsMode::Off,
        ..Default::default()
    };
    for ch in &mut m.ch {
        ch.on = true;
    }
    let sw = crate::sysconf::Switches { compress: false, ..Default::default() };
    let (input, frames, _) = scripted::draw_route_batch(&m, &sw, 0);
    assert_eq!(frames, 250);
    let raw = input.newest(0);
    let rate = (1.0 / raw.dt).round() as u64;
    let (fb, decode, codes) = (input.frame_bytes(), input.decode(), input.codes());
    let pipe = Pipe::start(0, 0, counters());
    let id = oos_core::change::Identity::default();
    pipe.publish_view(&m, 1024, 448, sw.interp, id);
    assert_eq!(crate::ui::batch_min_columns(), 271);
    let admitted = pipe.read_limit(id, Some((rate, 1))).unwrap() as usize;
    assert_eq!(admitted, 182);
    assert_eq!(standard::record_hit_bound(399, 3600.0, 0.0) * frames as u64, 2750);
    let bytes = input.into_bytes().bytes().to_vec();
    for columns in [271, 360, 399, 970, 1920] {
        for dots in [DotsMode::Off, DotsMode::On] {
            m.dots = dots;
            let batch = pipe.feed_batch(
                Box::new(super::super::tests::Bytes(bytes.clone())),
                fb,
                0..admitted,
                (0, Some(id)),
                decode.clone(),
                codes.clone(),
            );
            pipe.publish_view(&m, columns, 448, sw.interp, id);
            let view = batch_view(&batch, &oos_hal::sync::lock(&pipe.view), 0).unwrap();
            let mut painter = Painter::new(1, 0);
            let deposit = painter.batch(None, &pipe.slot, &batch, &view, Some(id)).unwrap();
            assert_eq!(deposit.records, admitted as u64);
            assert!(deposit.extent.hits() <= HIT_LIMIT);
            assert_eq!(pipe.counters.read().2, 0);
        }
    }
}

#[test]
fn raw_dot_envelope_stops_before_the_first_overflowing_hit_count() {
    let _scope = test_scope::Scope::new();
    let m = Model { dots: DotsMode::Off, ..Default::default() };
    let pipe = Pipe::start(0, 0, counters());
    let id = oos_core::change::Identity::default();
    pipe.publish_view(&m, 970, 448, crate::sparse::Interp::Exact, id);
    let window = m.time_per_div() as f64 * HDIV as f64;
    let rate = (970.0 * 84.75 / window).round() as u64;
    let hits = standard::dot_hit_bound(271.0 / (rate as f64 * window));
    let frames = u64::from(pipe.read_limit(id, Some((rate, 1))).unwrap());
    assert!(frames * hits <= HIT_LIMIT);
    assert!((frames + 1) * hits > HIT_LIMIT);
    // Pair compression cannot become raw dots without a new acquisition generation.
    assert!(pipe.read_limit(id, Some((rate, 16))).unwrap() > frames as u32);
}

#[test]
fn shipping_sparse_lines_plan_fits_all_five_hundred_records() {
    use super::super::tests::{batch, dec, model, Bytes};
    let (bytes, fb, s, p) = batch(500, 1408, false);
    let pipe = Pipe::start(0, 0, Box::leak(Box::default()));
    let m = model(1e-7, false, false);
    pipe.set_view(Some(&m), 970, 448, crate::sparse::Interp::Exact);
    let view = oos_hal::sync::lock(&pipe.view).view.clone().unwrap();
    let batch = pipe.feed_batch(Box::new(Bytes(bytes)), fb, 0..500, (0, None), dec(&s, &p), None);
    let mut painter = Painter::new(1, 0);
    let hits = painter.plan(&batch, &view, (0..500).step_by(SLICE)).0;
    assert_eq!(hits, 2500, "the legacy line rasterizer can reach five spans per column");
    assert_eq!(hits, 500 * view.record_hit_bound());
    let deposit = painter.batch(None, &pipe.slot, &batch, &view, None).unwrap();
    assert_eq!((deposit.records, deposit.extent.hits()), (500, hits));
    assert_eq!(pipe.counters.read().2, 0);
}

#[test]
fn acquisition_and_epoch_mismatches_are_settings_losses_not_capacity_losses() {
    use super::super::tests::{batch, dec, Bytes};
    use oos_core::change::{Request, Shadow, UiAcq};
    for epoch in [false, true] {
        let (bytes, fb, settings, params) = batch(3, 400, false);
        let pipe = Pipe::start(0, 0, Box::leak(Box::default()));
        let mut m = Model::default();
        let config = crate::model::config();
        let mut shadow = Shadow::new(&m, UiAcq::default(), &config).unwrap();
        let id = shadow.identity();
        pipe.publish_view(&m, 970, 448, Interp::Exact, id);
        let batch = pipe.feed_batch(Box::new(Bytes(bytes)), fb, 0..3, (0, Some(id)), dec(&settings, &params), None);
        if !epoch {
            m.ch[0].offset = 1.0;
        }
        let requests = if epoch { &[Request::Clear][..] } else { &[] };
        let current = shadow.observe(&m, UiAcq::default(), requests, &config).unwrap().identity();
        assert_ne!((id.gen(), id.epoch()), (current.gen(), current.epoch()));
        pipe.publish_view(&m, 271, 448, Interp::Exact, current);
        assert_eq!(pipe.read_limit(id, None), None);
        assert!(batch_view(&batch, &oos_hal::sync::lock(&pipe.view), 0).is_none());
        let counts = pipe.counters.records_text();
        assert!(counts.contains("lost.settings=3"), "{counts}");
        assert!(counts.contains("lost.budget=0"), "{counts}");
    }
}

#[test]
fn legal_wide_dots_remap_before_claim_but_normal_lines_keep_narrow_storage() {
    use oos_core::change::Identity;
    for dots in [DotsMode::Off, DotsMode::On] {
        let mut m = Model { dots, ..Default::default() };
        m.tscale = tscales().iter().position(|&t| (t - 500e-6).abs() < 1e-10).unwrap();
        m.mdepth = mdepths().iter().position(|d| d.1 == 1_000_000).unwrap();
        for ch in &mut m.ch {
            ch.on = true;
        }
        let sw = crate::sysconf::Switches { compress: false, ..Default::default() };
        let (input, frames, _) = scripted::draw_route_batch(&m, &sw, 1);
        let (fb, decode, codes) = (input.frame_bytes(), input.decode(), input.codes());
        let pipe = Pipe::start(0, 0, counters());
        let id = Identity::default();
        pipe.publish_view(&m, 970, 448, sw.interp, id);
        let batch = pipe.feed_batch(Box::new(input.into_bytes()), fb, 0..frames, (0, Some(id)), decode, codes);
        pipe.publish_view(&m, 271, 448, sw.interp, id);
        let view = batch_view(&batch, &oos_hal::sync::lock(&pipe.view), 0).unwrap();
        let mut painter = Painter::new(1, 0);
        let d = painter.batch(None, &pipe.slot, &batch, &view, Some(id)).unwrap();
        assert_eq!(d.records, frames as u64);
        assert_eq!(d.format(), if dots == DotsMode::On { CountFormat::Wide } else { CountFormat::Narrow });
        assert_eq!(batch.next.load(Ordering::Relaxed), frames);
        assert_eq!(pipe.counters.read().2, 0);
    }
}
