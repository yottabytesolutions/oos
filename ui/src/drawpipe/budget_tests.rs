// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::{model::*, sparse::Interp, wave::Phosphor};

#[test]
#[ignore = "host Batch picture work: release, OOS_PERF_TESTS=1, one test thread"]
fn perf_guard_batch_picture_merge() {
    use crate::board::scripted::{self, Batch};
    use crate::picture::deposit::kernel::fold_work;
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
        // This signal occupies 113 rows on each channel. Prefix the sentinel row too,
        // then write only the occupied rows. A second fold or a full scan exceeds this.
        let expected_work = [114 * 4 * 970, 113 * 4 * 970];
        for round in 0..8 {
            pipe.feed_codes(bytes, fb, frames, 0, decode.clone(), codes.clone());
            bytes = host::finish_batch(&pipe, round).unwrap();
            let (got, work) = fold_work::count(|| pipe.accept(&mut ph, &mut true, &mut [0; 4], &mut (0.0, 0.0)));
            assert_eq!(got.frames as usize, frames);
            assert_eq!(work, expected_work, "one fold of the touched rows for tdiv={tdiv}");
        }
        println!(
            "BATCH_PICTURE_WORK channels=4 tdiv={tdiv} frames={frames} prefix={} writes={}",
            expected_work[0], expected_work[1]
        );
        assert!(ph.maps().iter().all(|map| map.iter().any(|&v| v > 0)));
        shutdown::close(&pipe, Instant::now() + crate::ui::lifecycle::test_wait::HANG_GUARD);
    }
}

#[test]
fn completed_deposits_rejected_by_key_or_epoch_enter_distinct_record_buckets() {
    use oos_core::change::{Request, Shadow, UiAcq};
    let (bytes, fb, settings, params) = tests::batch(3, 400, false);
    let decode = tests::dec(&settings, &params);
    for epoch in [false, true] {
        let counters = Box::leak(Box::default());
        let pipe = Pipe::start(1, 0, counters);
        let mut model = Model::default();
        let config = crate::model::config();
        let mut shadow = Shadow::new(&model, UiAcq::default(), &config).unwrap();
        let identity = shadow.identity();
        pipe.publish_view(&model, 4, 4, Interp::Exact, identity);
        let feed = |id, count| {
            pipe.feed_with(Box::new(tests::Bytes(bytes.clone())), fb, 0..count, id, decode.clone(), None, None);
            drop(host::finish_batch(&pipe, 0).unwrap());
        };
        feed(identity, 3);
        feed(identity, 3);
        let current = if epoch {
            shadow.observe(&model, UiAcq::default(), &[Request::Clear], &config).unwrap().identity()
        } else {
            model.ch[0].offset = 1.0;
            identity
        };
        pipe.publish_view(&model, 4, 4, Interp::Exact, current);
        feed(current, 1);
        assert_eq!(counters.read().2, 3, "the coordinator rejects the old open deposit");
        let mut ph = Phosphor::new(4, 4);
        ph.channel_mut(0).fill(17);
        let got = pipe.accept(&mut ph, &mut true, &mut [0; 4], &mut (0.0, 0.0));
        assert_eq!(got.frames, 0);
        assert_eq!(counters.read().2, 6, "the UI rejects the old pending deposit");
        assert_eq!(ph.maps()[0], [17; 16]);
        let text = counters.records_text();
        assert!(
            text.contains(if epoch {
                "rejected.stale_key=0 rejected.epoch=6"
            } else {
                "rejected.stale_key=6 rejected.epoch=0"
            }),
            "{text}"
        );
        let stale_pipe = if epoch { 6 } else { 0 };
        assert!(counters.snapshot().legacy_losses_text().contains(&format!(" stale_pipe={stale_pipe} ")));
        host::barrier(&pipe);
        assert_eq!(pipe.accept(&mut ph, &mut true, &mut [0; 4], &mut (0.0, 0.0)).frames, 1);
    }
}
