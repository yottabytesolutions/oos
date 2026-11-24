// SPDX-License-Identifier: GPL-2.0-or-later

use super::{finish, Bytes, HeaderProbe, Identity, Interp, Model, Phosphor, Pipe, Readout, Settings};
use crate::{
    board::PictureEligibility,
    drawpipe::{self, clock::StageClock, host::barrier, test_scope::Scope},
};
use oos_hal::dho::{hz, seq};
use std::time::{Duration, Instant};

const SIZE: (usize, usize) = (128, 64);

#[derive(Debug, PartialEq, Eq)]
struct Drain {
    pictures: [u64; 3],
    taps: [(u64, PictureEligibility); 2],
    counts: (u64, u64, u64),
}

#[test]
fn releasing_the_output_slot_publishes_the_retained_deposit_without_an_empty_batch() {
    let first = drain();
    assert_eq!(first, drain());
    assert_eq!(first.pictures, [1, 2, 0]);
    assert_eq!(first.taps, [(1, PictureEligibility::AlreadyDeposited), (3, PictureEligibility::AlreadyDeposited)]);
    assert_eq!(first.counts, (5, 0, 0));
}

fn drain() -> Drain {
    let _scope = Scope::new();
    let pipe = drawpipe::start_pipe(&Default::default());
    let mut model = Model::default();
    model.ch[0].on = true;
    pipe.publish_view(&model, SIZE.0, SIZE.1, Interp::Exact, Identity::default());
    let settings = Settings { depth: 1_000, window: None, ..Settings::default() };
    let p = hz::prep(settings.mask, seq::tdiv_fs(settings.tdiv), settings.depth, 0);
    let origin = Instant::now();
    for (sequence, count) in [(1, 1), (3, 2)] {
        let clock = StageClock::from(origin + Duration::from_millis(sequence));
        submit(pipe, (&settings, &p), (sequence, count, false), clock);
        barrier(pipe);
    }
    // The first picture is pending and the second is retained by the coordinator.
    submit(pipe, (&settings, &p), (5, 2, true), StageClock::from(origin + Duration::from_millis(5)).with_admission());
    assert!(!pipe.in_flight());
    assert_eq!(pipe.tap_count(), 2);
    let mut ph = Phosphor::new(SIZE.0, SIZE.1);
    let mut compose = || pipe.accept(&mut ph, &mut true, &mut [0; 4], &mut (0.0, 0.0)).frames;
    let first = compose();
    barrier(pipe);
    let pictures = [first, compose(), compose()];
    assert!(!pipe.in_flight());
    let taps = std::array::from_fn(|_| {
        let tap = pipe.take_waveform(SIZE).unwrap();
        (tap.seq, tap.eligibility())
    });
    assert!(pipe.take_waveform(SIZE).is_none());
    let counters = drawpipe::counters();
    let text = counters.records_text();
    for expected in ["read=5 deposited=3", "lost.frame0=1", "lost.frame1=1", "lost.unclaimed=0"] {
        assert!(text.contains(expected), "{text}");
    }
    Drain { pictures, taps, counts: counters.read() }
}

fn submit(
    pipe: &Pipe,
    (settings, p): (&Settings, &hz::Params),
    (sequence, count, frame0_drop): (u64, u32, bool),
    clock: StageClock,
) {
    let fb = seq::frame_bytes(settings, p);
    let flat = 32_768_u16.to_le_bytes();
    let mut dma = Some(Bytes(flat.repeat(fb / 2 * count as usize)));
    crate::board::dho::publication::read(drawpipe::counters(), count, count, count as usize);
    let reservation = frame0_drop.then(|| {
        let storage = super::super::tap::storage(settings, fb).unwrap();
        pipe.reserve_waveform(&storage, Identity::default()).unwrap()
    });
    let charge = reservation.as_ref().map(std::sync::Arc::downgrade);
    let rest = Readout::new(
        pipe,
        (settings, p),
        (fb, count as usize, frame0_drop),
        (sequence, HeaderProbe::Off, None),
        (Identity::default(), clock),
    )
    .reserved(reservation)
    .dispatch(&mut dma, None, |frame| frame, &|| false);
    assert!(rest.is_empty());
    if let Some(charge) = charge {
        assert!(dma.is_some(), "empty selection leaves DMA with the source");
        assert!(charge.upgrade().is_none(), "empty selection refunds the tap reservation");
    } else {
        finish(pipe);
    }
}
