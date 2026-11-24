// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::{
    board::{Frame, PictureEligibility},
    model::*,
    picture::deposit::{Deposit, TapProjection},
    sparse::Interp,
    wave::Phosphor,
};
use oos_core::change::{Identity, Request, Shadow, UiAcq};
use std::sync::{atomic::Ordering, mpsc::channel, Arc};

#[test]
fn stop_wakes_a_legacy_waveform_reservation_and_preserves_the_queued_tap() {
    use crate::picture::deposit::Tap;
    use crate::ui::lifecycle::test_wait::HANG_GUARD;

    let _scope = test_scope::Scope::new();
    let sw = crate::sysconf::Switches::default();
    let pipe = start_pipe(&sw);
    let id = Identity::default();
    pipe.publish_view(&Model::default(), 8, 8, Interp::Exact, id);
    let submitted = oos_hal::sync::lock(&pipe.view).gen;
    let charge = pipe.slot.reserve_tap(crate::board::QUEUE_SAMPLES, submitted, Some(id)).unwrap();
    let frame = Arc::new(Frame { seq: 1, ..Default::default() });
    let tap = Tap { frame, identity: Some(id), projection: TapProjection::Unpainted, folds: None };
    pipe.slot.send_tap(tap, submitted, Some(&charge)).unwrap();
    drop(charge);

    let source = crate::wave::Source::Hw { shared: Default::default(), seen: 0 };
    let mut next = Frame::default();
    next.ch[0].push(0.0);
    let (done, result) = channel();
    std::thread::scope(|threads| {
        threads.spawn(|| done.send(pipe.reserve_waveform(&next, id).is_none()).unwrap());
        pipe.slot.wait_for_sender();
        source.hw_hold(true, 0);
        let stopped = result.recv_timeout(HANG_GUARD);
        if stopped.is_err() {
            pipe.slot.close();
        }
        assert!(stopped.expect("STOP releases the blocked reservation"));
    });
    assert_eq!(pipe.take_waveform((8, 8)).unwrap().seq, 1);
    assert!(pipe.reserve_waveform(&next, id).is_none());
    source.hw_hold(false, 0);
    assert!(pipe.reserve_waveform(&next, id).is_some());
}

pub(super) fn identities(request: Request) -> (Model, Identity) {
    let model = Model::default();
    let config = crate::model::config();
    let mut shadow = Shadow::new(&model, UiAcq::default(), &config).unwrap();
    let id = shadow.observe(&model, UiAcq::default(), &[request], &config).unwrap().identity();
    (model, id)
}

fn accumulator(gen: u64, identity: Identity) -> Deposit {
    let mut a = Deposit::new(gen, (8, 8), Some(identity), [true; 4]);
    a.records = 1;
    let mut fold = crate::framestats::Fold::default();
    fold.push(123.0);
    a.folds.push(((Meas::Vavg, 0), fold));
    a
}

#[test]
fn old_folds_cannot_refill_current_statistics() {
    for request in [Request::StatsReset, Request::Clear] {
        let (model, current) = identities(request);
        let counters = Box::leak(Box::<Counters>::default());
        let pipe = Pipe::start(1, 0, counters);
        pipe.set_view(Some(&model), 8, 8, Interp::Exact);
        let gen = pipe.view.lock().unwrap().gen;
        let mut phosphor = Phosphor::new(8, 8);
        Counters::add(&counters.placed, 1);
        assert!(pipe.slot.put(accumulator(gen, Identity::default())).is_ok());
        pipe.set_identity(current);
        let stale = pipe.accept(&mut phosphor, &mut false, &mut [0; 4], &mut (0.0, 0.0));
        assert!(stale.folds.is_empty(), "late folds must not refill reset statistics");
        let stale_pipe = u64::from(request == Request::Clear);
        assert!(counters.snapshot().legacy_losses_text().contains(&format!(" stale_pipe={stale_pipe} ")));
        assert_eq!(counters.losses.folds.load(Ordering::Relaxed), 1);
        assert_eq!(counters.losses.overflow.load(Ordering::Relaxed), 0);
        assert_eq!(stale.frames, u64::from(request == Request::StatsReset), "statistics alone retain valid pictures");
        Counters::add(&counters.placed, 1);
        assert!(pipe.slot.put(accumulator(gen, current)).is_ok());
        let valid = pipe.accept(&mut phosphor, &mut false, &mut [0; 4], &mut (0.0, 0.0));
        assert_eq!(valid.frames, 1);
        assert_eq!(valid.folds[0].1.n, 1, "valid current folds are retained");
    }
}

#[test]
fn rejecting_painted_planes_keeps_only_current_waveforms() {
    let (model, cleared) = identities(Request::Clear);
    let (_, stats_reset) = identities(Request::StatsReset);
    let current = Identity::default();
    let mut remapped = model.clone();
    remapped.tscale += 1;
    let waveform = Arc::new(Frame { seq: 1, ..Default::default() });
    for (view, view_width, picture_width, identity) in
        [(&remapped, 8, 8, stats_reset), (&model, 9, 8, current), (&model, 8, 9, current), (&model, 8, 8, cleared)]
    {
        let pipe = Pipe::start(0, 0, Box::leak(Box::default()));
        pipe.publish_view(&model, 8, 8, Interp::Exact, current);
        let gen = oos_hal::sync::lock(&pipe.view).gen;
        let deposit = accumulator(gen, current);
        assert!(pipe
            .slot
            .send_tap(
                crate::picture::deposit::Tap {
                    frame: waveform.clone(),
                    identity: Some(current),
                    projection: TapProjection::Projected { gen, size: (8, 8) },
                    folds: None,
                },
                gen,
                None,
            )
            .is_some());
        assert!(pipe.slot.put(deposit).is_ok());

        pipe.publish_view(view, view_width, 8, Interp::Exact, identity);
        let mut picture = Phosphor::new(picture_width, 8);
        let mut clear = true;
        let accepted = pipe.accept(&mut picture, &mut clear, &mut [0; 4], &mut (0.0, 0.0));
        assert_eq!((accepted.frames, accepted.triggered, accepted.newest), (0, 0, None));
        assert!(clear && accepted.folds.is_empty());
        assert_eq!(pipe.counters.losses.folds.load(Ordering::Relaxed), 1);
        let tap = pipe.take_waveform((picture_width, 8));
        if identity == cleared {
            assert!(tap.is_none());
        } else {
            let tap = tap.unwrap();
            assert!(Arc::ptr_eq(&tap, &waveform));
            assert_eq!(tap.eligibility(), PictureEligibility::ShownOnly);
        }
    }
}

#[test]
fn coalesced_deposit_leaves_an_interleaved_unpainted_tap_drawable() {
    let pipe = Pipe::start(0, 0, Box::leak(Box::default()));
    let model = Model::default();
    let id = Identity::default();
    let size = (8, 8);
    pipe.publish_view(&model, size.0, size.1, Interp::Exact, id);
    let gen = oos_hal::sync::lock(&pipe.view).gen;
    let mut deposit = accumulator(gen, id);
    deposit.records = 3;
    for seq in 1..=4 {
        let painted = seq != 2;
        let tap = crate::picture::deposit::Tap {
            frame: Arc::new(Frame { seq, ..Default::default() }),
            identity: Some(id),
            projection: if painted { TapProjection::Projected { gen, size } } else { TapProjection::Unpainted },
            folds: None,
        };
        let number = pipe.slot.send_tap(tap, gen, None).unwrap();
        if painted {
            deposit.include_tap(number);
        }
    }
    assert!(pipe.slot.put(deposit).is_ok());
    let mut ph = Phosphor::new(size.0, size.1);
    assert_eq!(pipe.accept(&mut ph, &mut true, &mut [0; 4], &mut (0.0, 0.0)).frames, 3);

    let mut changed = model;
    changed.ch[0].unit = AmpUnit::A;
    pipe.publish_view(&changed, 8, 8, Interp::Exact, id);
    let tap = pipe.take_waveform(size).unwrap();
    assert_eq!((tap.seq, tap.eligibility()), (1, PictureEligibility::AlreadyDeposited));

    pipe.publish_view(&changed, 9, 8, Interp::Exact, id);
    use PictureEligibility::{Drawable, Redraw};
    for (seq, eligibility) in [(2, Drawable), (3, Redraw)] {
        let tap = pipe.take_waveform((9, 8)).unwrap();
        assert_eq!((tap.seq, tap.eligibility()), (seq, eligibility));
    }
    let (_, cleared) = identities(Request::Clear);
    pipe.set_identity(cleared);
    assert!(pipe.take_waveform((9, 8)).is_none(), "an accepted tap from the old epoch stays stale");
}

#[test]
fn remapping_a_private_deposit_keeps_its_waveform_without_a_newer_tap() {
    use super::tests::{batch, dec, model, Bytes};

    let model = model(1e-6, false, false);
    let (bytes, fb, settings, params) = batch(1, 400, false);
    let decode = dec(&settings, &params);
    let mut frame = Frame { seq: 1, ..Default::default() };
    decode(&bytes, &mut frame);
    let waveform = Arc::new(frame);
    let current = Identity::default();
    let (_, cleared) = identities(Request::Clear);
    for identity in [current, cleared] {
        let pipe = Pipe::start(1, 0, Box::leak(Box::default()));
        pipe.publish_view(&model, 97, 48, Interp::Exact, current);
        for tap in [None, Some(waveform.clone())] {
            pipe.feed_with(Box::new(Bytes(bytes.clone())), fb, 0..1, current, decode.clone(), None, tap);
            host::finish_batch(&pipe, 0).unwrap();
        }
        pipe.publish_view(&model, 98, 48, Interp::Exact, identity);
        pipe.feed_with(Box::new(Bytes(bytes.clone())), fb, 0..0, identity, decode.clone(), None, None);
        host::finish_batch(&pipe, 0).unwrap();

        let mut picture = Phosphor::new(98, 48);
        assert_eq!(pipe.accept(&mut picture, &mut true, &mut [0; 4], &mut (0.0, 0.0)).frames, 0);
        host::barrier(&pipe);
        let accepted = pipe.accept(&mut picture, &mut true, &mut [0; 4], &mut (0.0, 0.0));
        assert_eq!(accepted.frames, 0);
        let tap = pipe.take_waveform((98, 48));
        if identity == current {
            let tap = tap.unwrap();
            assert!(Arc::ptr_eq(&tap, &waveform));
            assert_eq!(tap.eligibility(), PictureEligibility::ShownOnly);
        } else {
            assert!(tap.is_none());
        }
    }
}

#[test]
fn warm_pool_refreshes_measurements_after_a_statistics_change() {
    let (_, current) = identities(Request::StatsReset);
    let mut model = super::tests::model(1e-6, true, false);
    model.meas = vec![(Meas::Vpp, WaveId::Ch(0))];
    let pipe = Pipe::start(2, 0, Box::leak(Box::default()));
    let (bytes, fb, settings, params) = super::tests::batch(10, 12_512, true);
    for (id, kind, channel) in [(Identity::default(), Meas::Vpp, 0), (current, Meas::Vavg, 1)] {
        model.meas = vec![(kind, WaveId::Ch(channel))];
        pipe.publish_view(&model, 97, 48, Interp::Exact, id);
        pipe.feed_with(
            Box::new(super::tests::Bytes(bytes.clone())),
            fb,
            0..10,
            id,
            super::tests::dec(&settings, &params),
            None,
            None,
        );
        super::host::finish_batch(&pipe, 1).unwrap();
        let accepted = pipe.accept(&mut Phosphor::new(97, 48), &mut true, &mut [0; 4], &mut (0.0, 0.0));
        assert_eq!(accepted.frames, 10);
        assert_eq!(accepted.folds.len(), 1);
        assert_eq!(accepted.folds[0].0, (kind, channel));
        assert_eq!(accepted.folds[0].1.n, 10);
    }
    assert_eq!(pipe.counters.read().2, 0);
}
