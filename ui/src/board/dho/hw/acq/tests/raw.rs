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

#[test]
fn windowed_dma_keeps_a_whole_newest_replay_and_reports_the_batch_geometry() {
    let _scope = Scope::new();
    let settings = Settings { tdiv: 10e-6, depth: 100_000, compress: 15, ..settings() };
    let out = requested(&settings, Identity::default(), false);
    let (mut io, trace) = sim::fixture(&settings, out.clone(), (Run::Selection, 2, false), None);
    let (mut source, _) = source(&out, &io);
    source.switches = Switches::parse(&|_| None, "acq.play_window = on", "acq.frame0 = keep", Default::default());
    capture(&mut source, &mut io);
    let events = trace.borrow();
    let reads: Vec<_> = events
        .events
        .iter()
        .filter_map(|event| match event {
            Event::Dma(bytes) => Some(*bytes),
            _ => None,
        })
        .collect();
    assert_eq!(reads, [2 * 8912, 200_000]);
    let src = crate::wave::Source::Hw { shared: out.clone(), seen: 0 };
    let state = crate::board::board().capture_state(&src);
    assert!(state.contains("acq.play.lo=16560 acq.play.n=66840 acq.play.k=15"), "{state}");
    let shared = lock(&out);
    assert_eq!(shared.queue.len(), 2);
    let newest = &shared.queue[1];
    assert!(!newest.envelope);
    assert_eq!((newest.ch[0].len(), newest.pre), (100_000, 50_000));
    assert_eq!(
        (newest.cfg, newest.stamp),
        (shared.acquisition.identity().gen(), shared.acquisition.identity().epoch())
    );
    let counts = RecordCounts::read();
    assert_eq!((counts.read, counts.dropped), (2, 0));
}

#[test]
fn singleton_work_grant_plays_the_crop_and_still_reads_the_newest_raw() {
    let _scope = Scope::new();
    let settings = Settings { tdiv: 10e-6, depth: 100_000, compress: 15, ..settings() };
    let id = Identity::default();
    let out = requested(&settings, id, false);
    {
        let mut shared = lock(&out);
        shared.record_flow.cost.set_identity(id);
        shared.record_flow.cost.observe(id, 1, Duration::from_millis(50), Duration::ZERO);
    }
    let (mut io, trace) = sim::fixture(&settings, out.clone(), (Run::Selection, 1, false), None);
    let (mut source, _) = source(&out, &io);
    source.switches = Switches::parse(&|_| None, "acq.play_window = on", "acq.frame0 = keep", Default::default());
    capture(&mut source, &mut io);
    assert!(source.configured.play_window.is_some_and(|(_, window)| window.is_some()));
    let reads: Vec<_> = trace
        .borrow()
        .events
        .iter()
        .filter_map(|event| if let Event::Dma(bytes) = event { Some(*bytes) } else { None })
        .collect();
    assert_eq!(reads, [8_912, 200_000]);
}
