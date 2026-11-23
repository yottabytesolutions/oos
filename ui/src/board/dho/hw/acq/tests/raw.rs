// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::board::PictureEligibility;

pub(super) fn script(source: &mut AcqLoop<FixedPacing>, io: &mut Io<sim::Sim>) {
    for _ in 0..2 {
        capture(source, io);
        io.clock.compose();
    }
    io.clock.wait(crate::compress::RAW_PERIOD);
    capture(source, io);
    io.clock.compose();
}

#[test]
fn live_compression_taps_use_pairs_until_another_raw_replay_is_due() {
    let compressed = Settings { tdiv: 10e-6, depth: 14_000, compress: 4, ..settings() };
    let result = twice(|| run(Run::CompressedTap, (4, 4), compressed.clone(), (false, true), true));
    let taps: Vec<_> = result
        .events
        .iter()
        .filter_map(|event| match event {
            Event::Picture(picture) => Some(picture.tap.as_ref().unwrap()),
            _ => None,
        })
        .collect();
    let [raw, pairs, replay] = taps.as_slice() else { panic!("three pictures each carry one newest tap") };
    for (tap, (sequence, prior)) in taps.iter().zip([(4, 0), (8, 4), (12, 8)]) {
        assert_eq!(tap.frame, Frame::live(sequence, 3, prior));
        assert_eq!(tap.eligibility, PictureEligibility::AlreadyDeposited);
    }
    assert!(!raw.envelope && pairs.envelope && !replay.envelope);
    assert_eq!(raw.points, replay.points);
    assert!(raw.points > pairs.points);
}
