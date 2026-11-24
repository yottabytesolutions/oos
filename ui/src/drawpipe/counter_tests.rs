// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::{model::*, sparse::Interp, wave::Phosphor};
use std::sync::{atomic::Ordering, Arc};

#[test]
fn local_counts_and_sample_wait_for_existing_publication_and_picture_boundaries() {
    use crate::board::scripted::{self, Batch};
    let counters = &*Box::leak(Box::<Counters>::default());
    let pipe = Pipe::start(2, 0, counters);
    let model = Model { meas: vec![(Meas::Vavg, WaveId::Ch(0))], ..Model::default() };
    pipe.set_view(Some(&model), 16, 16, Interp::Exact);
    let (fixture, _, _) = scripted::draw_route_batch(&model, &Default::default(), 65);
    let (fb, decode) = (fixture.frame_bytes(), fixture.decode());
    let first_decode = decode.clone();
    let check_decode: DecodeInto = Arc::new(move |bytes, frame| {
        assert_eq!(counters.deposited.load(Ordering::Relaxed), 0);
        assert_eq!(counters.measured.load(Ordering::Relaxed), 0);
        first_decode(bytes, frame);
    });
    // The first batch spans two placement slices. The second slice sees no counts from the first.
    pipe.feed(Box::new(fixture.into_bytes()), fb, 65, 0, check_decode);
    let mut bytes = host::finish_batch(&pipe, 0).unwrap();
    assert_eq!(counters.deposited.load(Ordering::Relaxed), 65);
    assert_eq!(counters.measured.load(Ordering::Relaxed), 65);
    for _ in 0..62 {
        pipe.feed(bytes, fb, 1, 0, decode.clone());
        bytes = host::finish_batch(&pipe, 0).unwrap();
    }
    assert_eq!(counters.deposited.load(Ordering::Relaxed), 127);
    assert_eq!(counters.measured.load(Ordering::Relaxed), 127);
    let mut ph = Phosphor::new(16, 16);
    assert_eq!(pipe.accept(&mut ph, &mut false, &mut [0; 4], &mut (0.0, 0.0)).frames, 65);
    assert!(counters.timing.text().contains("deposit.slice n=0"), "the sample waits for its picture");
    host::barrier(&pipe);
    assert_eq!(pipe.accept(&mut ph, &mut false, &mut [0; 4], &mut (0.0, 0.0)).frames, 62);
    let text = counters.timing.text();
    for ring in ["slice", "record"] {
        assert!(text.contains(&format!("deposit.{ring} n=1 total=1")), "{text}");
    }
    assert!(text.contains("deposit.batch n=2 total=2"), "{text}");
}

#[test]
fn obsolete_batches_count_as_settings_losses_before_deposition() {
    let (_, current) = super::identity_tests::identities(oos_core::change::Request::Clear);
    let counters = Box::leak(Box::<Counters>::default());
    let pipe = Pipe::start(1, 0, counters);
    pipe.publish_view(&Model::default(), 16, 16, Interp::Exact, current);
    let (bytes, fb, settings, params) = tests::batch(3, 400, false);
    pipe.feed_with(
        Box::new(tests::Bytes(bytes)),
        fb,
        0..3,
        Default::default(),
        tests::dec(&settings, &params),
        None,
        None,
    );
    drop(host::finish_batch(&pipe, 0).unwrap());
    assert_eq!(counters.read().2, 3);
    assert!(counters.snapshot().legacy_losses_text().contains(" stale_pipe=3 "));
    let text = counters.records_text();
    for field in ["deposited=0", "lost.settings=3", "lost.unclaimed=0", "rejected.epoch=0"] {
        assert!(text.contains(field), "{text}");
    }
}

#[test]
fn intentional_selection_keeps_dropped_separate_from_acquired_minus_used() {
    let counters = Counters::default();
    Counters::add(&counters.acquired, 10);
    Counters::add(&counters.used, 3);
    counters.lose(Lost::Frame0, 1);
    counters.lose(Lost::Average, 4);
    assert_eq!(counters.read(), (10, 3, 0));
    assert!(counters.records_text().contains("not_used=7"));
    counters.discard(Lost::Unclaimed, 2);
    assert_eq!(counters.read(), (10, 3, 2));
    assert!(counters.records_text().contains("not_used=7"), "pending records remain in the gap");
}

#[test]
fn record_snapshot_keeps_legacy_and_detailed_totals_together() {
    let counters = Counters::default();
    Counters::add(&counters.acquired, 21);
    Counters::add(&counters.used, 7);
    Counters::add(&counters.losses.worker_push, 1);
    Counters::add(&counters.losses.ui_publish, 2);
    Counters::add(&counters.losses.queue_pop, 3);
    counters.reject_batch(true, 4);
    counters.reject_deposit(None, Some(Default::default()), 5);
    counters.discard(Lost::Settings, 6);
    counters.lose(Lost::Settings, 7);
    Counters::add(&counters.losses.overflow, 8);
    let snapshot = counters.snapshot();
    Counters::add(&counters.acquired, 10);
    Counters::add(&counters.losses.overflow, 11);
    Counters::add(&counters.losses.worker_push, 12);
    assert_eq!(snapshot.legacy_totals(), (21, 7, 29, 0));
    assert_eq!(
        snapshot.legacy_losses_text(),
        " stale_worker_push=1 stale_ui_publish=2 stale_queue_pop=3 stale_pipe=9 queue_overflow=8 fold_drops=0"
    );
    let records = snapshot.records_text();
    assert!(records.contains(" read=21 "));
    assert!(records.contains(" not_used=14 lost.settings=23 "));
    assert!(records.ends_with(" lost.queue_overflow=8 rejected.stale_key=0 rejected.epoch=5"));
}
