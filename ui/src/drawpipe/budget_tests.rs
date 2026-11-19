// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

#[test]
fn accumulator_storage_follows_enabled_channels_and_releases_disabled_capacity() {
    let pipe = Pipe::start(0, 0, Box::leak(Box::default()));
    let mut model = Model::default();
    let mut spare = None;
    for channels in [4, 1, 2, 4, 1] {
        for (c, ch) in model.ch.iter_mut().enumerate() {
            ch.on = c < channels;
        }
        pipe.set_view(Some(&model), 970, 448, Interp::Exact);
        let view = pipe.view_now().0.unwrap();
        let acc = fresh_acc(spare.take(), &view);
        assert_eq!(acc.ph.map.iter().map(Vec::len).sum::<usize>(), channels * 970 * 448);
        assert!(acc.ph.map.iter().skip(channels).all(|m| m.capacity() == 0));
        spare = Some(acc);
    }
}

#[test]
fn empty_completion_keeps_newer_visible_pixels_and_clear_intent() {
    let pipe = Pipe::start(0, 0, Box::leak(Box::default()));
    pipe.set_view(Some(&Model::default()), 4, 4, Interp::Exact);
    let view = pipe.view_now().0.unwrap();
    // A completed batch can publish its newest record without any deposited neighbours.
    oos_hal::sync::lock(&pipe.mails[0]).full.push(fresh_acc(None, &view));
    let mut ph = Phosphor::new(4, 4);
    ph.map[0].fill(17);
    let (mut clear, mut edges, mut off) = (true, [0; 4], (0.0, 0.0));
    let completed = Arc::new(AtomicU64::new(0));
    let delivered = completed.clone();
    oos_hal::sync::lock(&pipe.ready).push(Box::new(move || {
        delivered.fetch_add(1, Ordering::Relaxed);
    }));
    let got = pipe.merge(&mut ph, &mut clear, &mut edges, &mut off);
    assert_eq!(got.frames, 0);
    assert_eq!(ph.map[0], [17; 16]);
    assert!(clear);
    assert_eq!(completed.load(Ordering::Relaxed), 1);
    assert_eq!((edges, off), ([0; 4], (0.0, 0.0)));
}

#[test]
#[ignore = "host Batch picture baseline: release, OOS_PERF_TESTS=1, one test thread"]
fn perf_guard_batch_picture_merge() {
    use crate::board::scripted::{self, Batch};
    use std::time::Instant;
    assert_eq!(std::env::var("OOS_PERF_TESTS").as_deref(), Ok("1"));
    for (tdiv, frames) in [(1e-6, 90), (10e-6, 14)] {
        let mut model = Model {
            tscale: tscales().iter().position(|&v| (v - tdiv).abs() < tdiv * 1e-4).unwrap(),
            acq: AcqMode::Normal,
            dots: DotsMode::Off,
            meas: Vec::new(),
            ..Model::default()
        };
        for channel in &mut model.ch {
            channel.on = true;
        }
        let pipe = Pipe::start(4, 0, Box::leak(Box::default()));
        pipe.set_view(Some(&model), 970, 448, Interp::Exact);
        let (fixture, frames, _) = scripted::draw_route_batch(&model, &Default::default(), frames);
        let (fb, decode, codes) = (fixture.frame_bytes(), fixture.decode(), fixture.codes());
        let mut bytes: Box<dyn RawBatch> = Box::new(fixture.into_bytes());
        let mut ph = Phosphor::new(970, 448);
        let mut times = Vec::new();
        for round in 0..crate::perfguard::ROUNDS + 8 {
            pipe.feed_codes(bytes, fb, frames, 0, decode.clone(), codes.clone());
            bytes = host::finish_batch(&pipe, round as u64).unwrap();
            let start = Instant::now();
            let got = pipe.merge(&mut ph, &mut true, &mut [0; 4], &mut (0.0, 0.0));
            let elapsed = start.elapsed().as_nanos();
            assert_eq!(got.frames as usize, frames);
            if round >= 8 {
                times.push(elapsed);
            }
        }
        times.sort_unstable();
        println!("BATCH_PICTURE channels=4 acq=Normal tdiv={tdiv} frames={frames} merge_ns_p50={} merge_ns_p90={} merge_ns_p99={} ui_ns_p50={} ui_ns_p99={}", times[50], times[90], times[99], times[50], times[99]);
        assert!(ph.map.iter().all(|map| map.iter().any(|&v| v > 0)));
        assert!(times[99] < 16_500_000, "merge must fit within half a picture period on the host");
        shutdown::close(&pipe, Instant::now() + crate::ui::lifecycle::test_wait::HANG_GUARD);
    }
}

#[test]
fn completed_maps_rejected_by_key_or_epoch_enter_distinct_record_buckets() {
    use oos_core::change::{Request, Shadow, UiAcq};
    for (epoch, worker_publish) in [(false, false), (false, true), (true, false), (true, true)] {
        let counters = Box::leak(Box::default());
        let pipe = Pipe::start(0, 0, counters);
        let mut model = Model::default();
        let config = crate::model::config();
        let mut shadow = Shadow::new(&model, UiAcq::default(), &config).unwrap();
        let identity = shadow.identity();
        pipe.publish_view(&model, 4, 4, Interp::Exact, identity);
        let mut share = fresh_acc(None, &pipe.view_now().0.unwrap());
        share.identity = Some(identity);
        share.frames = 3;
        oos_hal::sync::lock(&pipe.mails[0]).full.push(share);
        if epoch {
            let id = shadow.observe(&model, UiAcq::default(), &[Request::Clear], &config).unwrap().identity();
            pipe.set_identity(id);
        } else {
            model.ch[0].offset = 1.0;
            pipe.publish_view(&model, 4, 4, Interp::Exact, identity);
        }
        if worker_publish {
            let mut current = fresh_acc(None, &pipe.view_now().0.unwrap());
            current.identity = oos_hal::sync::lock(&pipe.view).identity;
            publish(&pipe.mails[0], current, counters);
        }
        let mut ph = Phosphor::new(4, 4);
        ph.map[0].fill(17);
        let got = pipe.merge(&mut ph, &mut true, &mut [0; 4], &mut (0.0, 0.0));
        assert_eq!(got.frames, 0);
        assert_eq!(counters.read().2, 3);
        assert_eq!(ph.map[0], [17; 16]);
        let text = counters.records_text();
        assert!(
            text.contains(if epoch {
                "rejected.stale_key=0 rejected.epoch=3"
            } else {
                "rejected.stale_key=3 rejected.epoch=0"
            }),
            "{text}"
        );
    }
}
