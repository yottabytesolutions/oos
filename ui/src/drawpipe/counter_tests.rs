// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

#[test]
fn local_counts_and_sample_wait_for_existing_publication_and_picture_boundaries() {
    use crate::board::scripted::{self, Batch};
    let counters = Box::leak(Box::default());
    let pipe = Pipe::start(0, 0, counters);
    let model = Model { meas: vec![(Meas::Vavg, WaveId::Ch(0))], ..Model::default() };
    pipe.set_view(Some(&model), 16, 16, Interp::Exact);
    let (fixture, _, _) = scripted::draw_route_batch(&model, &Default::default(), 64);
    let (fb, decode) = (fixture.frame_bytes(), fixture.decode());
    let _batch = pipe.feed_batch(Box::new(fixture.into_bytes()), fb, 0..64, (0, None), decode, None);
    let mut helper = Helper::default();
    for _ in 0..64 {
        assert!(pipe.help(&mut helper));
        assert_eq!(counters.deposited.load(Ordering::Relaxed), 0);
        assert_eq!(counters.measured.load(Ordering::Relaxed), 0);
    }
    pipe.help_done(&mut helper);
    assert_eq!(counters.deposited.load(Ordering::Relaxed), 64);
    assert_eq!(counters.measured.load(Ordering::Relaxed), 64);
    assert!(counters.timing.text().contains("deposit.slice n=0"), "producers do not touch shared timing rings");
    let got = pipe.merge(&mut Phosphor::new(16, 16), &mut false, &mut [0; 4], &mut (0.0, 0.0));
    assert_eq!(got.frames, 64);
    let text = counters.timing.text();
    for ring in ["slice", "record", "batch"] {
        assert!(text.contains(&format!("deposit.{ring} n=1 total=1")), "{text}");
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
