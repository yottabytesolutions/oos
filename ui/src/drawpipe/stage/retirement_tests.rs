// SPDX-License-Identifier: GPL-2.0-or-later

use super::super::tests::{model, Bytes};
use super::*;
use std::time::Duration;

fn fixture() -> (Pipe, Arc<Batch>, Arc<View>) {
    let pipe = Pipe::start(0, 0, Box::leak(Box::default()));
    let mut m = model(1e-6, false, false);
    m.run = RunState::Run;
    m.trig_mode = TrigMode::Normal;
    m.persist = 0;
    pipe.set_view(Some(&m), 97, 48, Interp::Exact);
    let view = oos_hal::sync::lock(&pipe.view).view.clone().unwrap();
    let decode: DecodeInto = Arc::new(|_, _| {});
    let batch = pipe.feed_batch(Box::new(Bytes(vec![0])), 1, 0..1, (m.hw_stamp, None), decode, None);
    (pipe, batch, view)
}

#[test]
fn delayed_open_deposit_keeps_records_and_taps_when_the_next_batch_merges() {
    let (pipe, b, v) = fixture();
    let mut old = Deposit::new(v.gen, (v.w, v.h), b.header.identity(), v.m.ch.each_ref().map(|c| c.on));
    old.records = 7;
    old.triggered = 3;
    let frame = Arc::new(Frame::default());
    let tap = || crate::picture::deposit::Tap {
        frame: frame.clone(),
        identity: None,
        projection: TapProjection::Projected { gen: v.gen, size: (v.w, v.h) },
        folds: None,
    };
    let first = pipe.slot.send_tap(tap(), v.gen, None).unwrap();
    let last = pipe.slot.send_tap(tap(), v.gen, None).unwrap();
    old.include_tap(first);
    old.include_tap(last);
    let tap_view = TapView::new(v.gen, v.gen, (v.w, v.h));
    assert!(matches!(pipe.slot.take_tap(Some(tap_view), None, None, true), TapRead::Ready(_)));
    old.extent.include(b.clock.read_at() - Duration::from_secs(1));
    old.planes[0].test_span(3, 24);
    let backing = old.planes[0].plane.d.as_ptr();
    let mut next = preparation::deposit(Some(old), &pipe.slot, &b, &v, None, (1, CountFormat::Narrow)).unwrap();
    assert_eq!(next.records, 7);
    assert_eq!(next.triggered, 3);
    assert_eq!(next.taps, first..last + 1);
    assert_eq!(next.planes[0].plane.d.as_ptr(), backing, "reuse the exact plane backing");
    next.planes[0].test_span(5, 48);
    next.records += 2;
    next.extent.include(b.clock.read_at());
    assert!(pipe.slot.put(next).is_ok());
    let mut ph = Phosphor::new(v.w, v.h);
    let mut window = DepositWindow::default();
    let accepted =
        pipe.accept_into(&mut ph, &mut true, Stats { edges: &mut [0; 4], off: &mut (0.0, 0.0) }, Some(&mut window));
    assert_eq!(accepted.frames, 9);
    assert_eq!(pipe.counters.read().2, 0);
    assert_eq!(pipe.take_waveform((v.w, v.h)).unwrap().eligibility(), PictureEligibility::AlreadyDeposited);
    assert!(ph.maps()[0][3 * v.w..4 * v.w].iter().any(|&n| n > 0));
    assert!(ph.maps()[0][5 * v.w..6 * v.w].iter().any(|&n| n > 0));
    assert_eq!(pipe.accept(&mut ph, &mut true, &mut [0; 4], &mut (0.0, 0.0)).frames, 0);
}
