// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::board::scripted::{self, Batch as _};
use crate::drawpipe::budget::WORK_BUDGET;
use crate::system::test_allocator::own_high_water;

#[test]
fn coalesced_records_do_not_dilute_the_current_batch_cost() {
    use super::super::{
        clock::StageClock,
        tests::{batch, dec, model, Bytes},
    };
    let pipe = Pipe::start(0, 0, Box::leak(Box::default()));
    let id = oos_core::change::Identity::default();
    pipe.publish_view(&model(1e-6, false, false), 97, 48, Interp::Exact, id);
    let view = oos_hal::sync::lock(&pipe.view).view.clone().unwrap();
    let (bytes, fb, settings, params) = batch(2, 400, false);
    let feed = |count| {
        pipe.feed_batch(Box::new(Bytes(bytes.clone())), fb, 0..count, (0, Some(id)), dec(&settings, &params), None)
    };
    let mut painter = Painter::new(1, 0);
    let current = painter.batch(None, &pipe.slot, &feed(2), &view, Some(id));
    let mut next = feed(1);
    Arc::get_mut(&mut next).unwrap().clock =
        StageClock::from(Instant::now()).with_admission().pool_for_test(Duration::from_millis(2));
    *oos_hal::sync::lock(&next.header.flight) = Some(pipe.admission.submit(&pipe.source, id, pipe.counters));
    let deposit = painter.batch(current, &pipe.slot, &next, &view, Some(id)).unwrap();
    assert_eq!(deposit.records, 3);
    assert!(owner::publish_completed(&pipe.slot, next, deposit, &pipe.view, pipe.counters).is_none());
    assert_eq!(pipe.grant(id, 100, WORK_BUDGET).records(), 6, "one record used 2 ms of pool work");
}

#[test]
fn short_batches_decode_only_records_without_a_supplied_waveform() {
    use super::super::tests::{model, Bytes};

    for frames in [1, 2] {
        let pipe = Pipe::start(1, 0, Box::leak(Box::default()));
        let m = model(1e-6, false, false);
        let id = oos_core::change::Identity::default();
        let (w, h) = (97, 48);
        pipe.publish_view(&m, w, h, Interp::Exact, id);
        let mut newest = Frame { dt: 4e-9, pre: 150, ..Default::default() };
        newest.ch[0] = vec![0.5; 300];
        let newest = Arc::new(newest);
        let calls = Arc::new(Mutex::new(Vec::new()));
        let decode: IndexedDecodeInto = Arc::new({
            let calls = calls.clone();
            let template = newest.clone();
            move |index, _, frame| {
                oos_hal::sync::lock(&calls).push(index);
                frame.clone_from(&template);
            }
        });
        let raw = Box::new(Bytes(vec![0; frames]));
        assert!(pipe
            .feed_readout((raw, 1, 0..frames), (id, std::time::Instant::now()), decode, None, (Some(newest), None))
            .is_none());
        host::finish_batch(&pipe, 0).unwrap();
        let accepted = pipe.accept(&mut Phosphor::new(w, h), &mut true, &mut [0; 4], &mut (0.0, 0.0));
        assert_eq!(accepted.frames, frames as u64);
        assert_eq!(*oos_hal::sync::lock(&calls), (0..frames - 1).collect::<Vec<_>>());
    }
}

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
    let newest = Arc::new(input.newest(m.hw_stamp));
    let mut bytes: Option<Box<dyn RawBatch>> = Some(Box::new(input.into_bytes()));
    // Run the production placement and tile jobs on the measuring thread.
    let pipe = Pipe::start(0, 0, counters());
    let (w, h) = (970, 448);
    pipe.set_view(Some(m), w, h, crate::sparse::Interp::Exact);
    let view = oos_hal::sync::lock(&pipe.view).view.clone().unwrap();
    let mut painter = Painter::new(1, 0);
    let mut ph = Phosphor::new(w, h);
    let mut picture = || {
        let mut batch =
            pipe.feed_batch(bytes.take().unwrap(), fb, 0..frames, (m.hw_stamp, None), decode.clone(), codes.clone());
        Arc::get_mut(&mut batch).unwrap().newest = Some(newest.clone());
        let deposit = painter.batch(None, &pipe.slot, &batch, &view, None).unwrap();
        assert_eq!(deposit.planes.iter().map(|b| b.plane.d.len()).sum::<usize>(), channels * w * (h + 1));
        assert!(deposit.planes.iter().all(|b| m.ch[b.channel].on));
        assert!(pipe.slot.put(deposit).is_ok());
        drop(batch);
        bytes = Some(host::finish_batch(&pipe, 0).unwrap());
        let got = pipe.accept(&mut ph, &mut true, &mut [0; 4], &mut (0.0, 0.0));
        assert_eq!(got.frames, frames as u64);
        assert!(Arc::ptr_eq(&pipe.take_waveform((w, h)).unwrap(), &newest));
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
    assert!(expected.maps()[0].iter().any(|&v| v > 0));
    assert_eq!(actual.maps(), expected.maps());
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
        let pipe = Pipe::start(0, 0, Box::leak(Box::default()));
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

#[test]
fn warmed_pool_placement_reuses_rows_envelopes_and_search_storage() {
    use route::Route;
    for (expected, mask, acq, tdiv, kind) in [
        (Route::Dense, 1u32, AcqMode::Normal, 1e-6, None),
        (Route::Dense, 0b1010, AcqMode::Normal, 1e-6, None),
        (Route::Dense, 0b1101, AcqMode::Normal, 1e-6, None),
        (Route::Dense, 15, AcqMode::Normal, 1e-6, None),
        (Route::Peak, 1, AcqMode::Peak, 1e-6, None),
        (Route::Boxcar, 15, AcqMode::HiRes, 10e-6, None),
        (Route::Upsampled, 1, AcqMode::Normal, 20e-9, None),
        (Route::Pairs, 1, AcqMode::Normal, 200e-6, Some(scripted::BatchKind::LivePairs)),
        (Route::Pairs, 1, AcqMode::Normal, 1e-3, Some(scripted::BatchKind::DeepPairs)),
    ] {
        let mut m = super::super::tests::model(tdiv, false, false);
        m.acq = acq;
        m.trig_src = mask.trailing_zeros() as usize;
        for (channel, config) in m.ch.iter_mut().enumerate() {
            config.on = mask & (1 << channel) != 0;
        }
        let (input, frames) = if let Some(kind) = kind {
            use scripted::BatchSource;
            (scripted::source(kind, 123).next_batch(&m, &Default::default(), 3, None).unwrap(), 3)
        } else {
            let (input, frames, _) = scripted::draw_route_batch(&m, &Default::default(), 3);
            (input, frames)
        };
        let (fb, decode, codes) = (input.frame_bytes(), input.decode(), input.codes());
        let pipe = Pipe::start(0, 0, Box::leak(Box::default()));
        pipe.set_view(Some(&m), 384, 112, Interp::Exact);
        let view = oos_hal::sync::lock(&pipe.view).view.clone().unwrap();
        let batch = pipe.feed_batch(Box::new(input.into_bytes()), fb, 0..frames, (0, None), decode, codes);
        let mut jobs: Vec<_> = (0..frames)
            .map(|index| {
                (
                    batch.clone(),
                    view.clone(),
                    Prepared {
                        fallbacks: Default::default(),
                        index,
                        decoded_dots: false,
                        plan: None,
                        codes: None,
                        folds: Vec::new(),
                        measured: false,
                        refine: Duration::ZERO,
                        screen: Default::default(),
                        zoom: None,
                    },
                )
            })
            .collect();
        let mut pool = Pool::new(1, 0, |scratch, (batch, view, prepared)| {
            place(batch, view, prepared, scratch);
        });
        pool.map(&mut jobs, true);
        let (allocations, _) = crate::system::test_allocator::count_own(|| pool.map(&mut jobs, true));
        assert_eq!(allocations, 0, "{expected:?} {mask:04b}");
        assert!(jobs.iter().all(|(_, _, p)| p.plan.as_ref().unwrap().route == expected), "{expected:?} {mask:04b}");
    }
}

#[test]
fn warmed_zoom_placement_and_tiles_allocate_nothing() {
    for dots in [DotsMode::Off, DotsMode::On] {
        let mut m = super::super::tests::model(10e-6, false, false);
        m.zoom = true;
        m.dots = dots;
        m.zscale = tscales().iter().position(|&t| t == 2e-6).unwrap();
        m.ch.iter_mut().for_each(|channel| channel.on = true);

        let pipe = Pipe::start(0, 0, Box::leak(Box::default()));
        let zoom = route::ZoomView::new((384, 144), 2.0, 0.25);
        pipe.publish_picture_view(&m, (384, 112), Interp::Exact, Default::default(), Some(zoom));
        let view = oos_hal::sync::lock(&pipe.view).view.clone().unwrap();
        let (input, frames, _) = scripted::draw_route_batch(&m, &Default::default(), 3);
        let (fb, decode, codes) = (input.frame_bytes(), input.decode(), input.codes());
        let batch = pipe.feed_batch(Box::new(input.into_bytes()), fb, 0..frames, (0, None), decode, codes);
        let mut painter = Painter::new(1, 0);
        let mut deposit = pipe.slot.acquire_picture(view.gen, view.picture_sizes(), None, [true; NCH]).unwrap();
        let mut paint = |deposit: &mut Deposit| {
            let count = painter.place_slice(&batch, &view, 0);
            painter.draw_tiles(&batch, deposit, count);
        };
        paint(&mut deposit);
        deposit.reset(None);
        let (allocations, ()) = crate::system::test_allocator::count_own(|| paint(&mut deposit));
        assert_eq!(allocations, 0, "{dots:?}");

        let mut main = Phosphor::new(384, 112);
        let mut detail = Phosphor::new(384, 144);
        deposit.sort_bands();
        deposit.blend_picture(&mut crate::picture::PerMap::new(&mut main, Some(&mut detail)), true);
        assert!(main.channel_max(0) > 0);
        assert!(detail.channel_max(0) > 0);
        assert_eq!(pipe.slot.backing_count(), 1);
    }
}
