// SPDX-License-Identifier: GPL-2.0-or-later
//! Whole first B-tap consumer step, with deterministic intake and no timing gate.
use super::*;

#[test]
fn configured_first_whole_b_step_consumes_every_owned_tap() {
    for tdiv in [1e-6, 10e-6] {
        for records in [1, 2] {
            let _scope = Scope::new();
            let sw =
                crate::sysconf::Switches::parse(&|_| None, drawpipe::funding::TEST_PROFILE, "", Default::default());
            let mut route = Route::new(BatchKind::Raw, true, 4, tdiv);
            route.ui.host = route.ui.host.with_switches(sw);
            let id = change::acquisition::script_prepare(&mut route.ui);
            let pipe = drawpipe::pipe(drawpipe::Access::Existing).unwrap();
            let input = route.source.next_batch(&route.ui.m, &sw, records, None).unwrap();
            let sample_bound = input.newest(id.epoch()).ch.iter().map(Vec::capacity).sum();
            assert!(pipe.admit_taps(id, [0; 6], sample_bound, std::time::Duration::from_micros(500)));
            let b = pipe.tap_drain();
            assert_eq!(b, 70);
            assert!(crate::cmd::stat_text(&route.ui).contains("pipeline Batch route=batch-funded"));
            for sequence in 0..b {
                let batch = route.source.next_batch(&route.ui.m, &sw, records, None).unwrap();
                let mut frame = batch.newest(id.epoch());
                (frame.seq, frame.cfg) = (sequence as u64 + 1, id.gen());
                let reservation = pipe.reserve_plan(id).unwrap();
                frame.basic.retain(reservation.clone());
                let (fb, decode, codes) = (batch.frame_bytes(), batch.decode(), batch.codes());
                pipe.feed_readout(
                    (batch.into_bytes().into(), fb, 0..records),
                    (id, route.ui.host.picture_time(route.ui.now)),
                    Arc::new(move |_, bytes, frame| decode(bytes, frame)),
                    codes,
                    (Some(Arc::new(frame)), Some(reservation)),
                );
                drawpipe::host::finish_batch(pipe, sequence as u64).unwrap();
            }
            assert_eq!(pipe.tap_count(), b);
            let (allocations, (peak_bytes, ())) = crate::system::test_allocator::count_own(|| {
                crate::system::test_allocator::own_high_water(|| route.ui.step(1.0 / 30.0))
            });
            assert_eq!(pipe.tap_count(), 0);
            assert_eq!(route.ui.history.len(), b);
            println!("F2_WHOLE_B_STEP tdiv={tdiv} records={records} B={b} taps_consumed={b} allocations={allocations} peak_thread_bytes={peak_bytes} max_native_samples_per_tap={sample_bound} timing_qualified=false");
        }
    }
}

#[test]
fn staged_status_reports_standard_until_current_geometry_is_admitted() {
    for conf in ["f2.stage=configured-evaluation", drawpipe::funding::TEST_PROFILE] {
        let _scope = Scope::new();
        let mut route = Route::new(BatchKind::Raw, true, 4, 10e-6);
        let sw = crate::sysconf::Switches::parse(&|_| None, conf, "", Default::default());
        route.ui.host = route.ui.host.with_switches(sw);
        let id = change::acquisition::script_prepare(&mut route.ui);
        let pipe = drawpipe::pipe(drawpipe::Access::Existing).unwrap();
        assert!(crate::cmd::stat_text(&route.ui).contains("pipeline Frames route=standard"));
        assert!(!pipe.admit_taps(id, [0; 6], usize::MAX, std::time::Duration::from_secs(1)));
        assert!(crate::cmd::stat_text(&route.ui).contains("pipeline Frames route=standard"));
        assert!(pipe.reserve_plan(id).is_none());
    }
}
