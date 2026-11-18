// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use oos_core::change::{Identity, Request, Shadow, UiAcq};

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
        pipe.set_identity(current);
        let gen = pipe.view.lock().unwrap().gen;
        let mut phosphor = Phosphor::new(8, 8);
        assert!(pipe.slot.put(accumulator(gen, Identity::default())).is_ok());
        let stale = pipe.accept(&mut phosphor, &mut false, &mut [0; 4], &mut (0.0, 0.0));
        assert!(stale.folds.is_empty(), "late folds must not refill reset statistics");
        assert_eq!(counters.losses.pipe.load(Ordering::Relaxed), u64::from(request == Request::Clear));
        assert_eq!(counters.losses.folds.load(Ordering::Relaxed), 1);
        assert_eq!(counters.losses.overflow.load(Ordering::Relaxed), 0);
        assert_eq!(stale.frames, u64::from(request == Request::StatsReset), "statistics alone retain valid pictures");
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
        let mut deposit = accumulator(oos_hal::sync::lock(&pipe.view).gen, current);
        deposit.newest = Some(waveform.clone());
        assert!(pipe.slot.put(deposit).is_ok());

        pipe.publish_view(view, view_width, 8, Interp::Exact, identity);
        let mut picture = Phosphor::new(picture_width, 8);
        let mut clear = true;
        let accepted = pipe.accept(&mut picture, &mut clear, &mut [0; 4], &mut (0.0, 0.0));
        assert_eq!((accepted.frames, accepted.triggered, accepted.newest), (0, 0, None));
        assert!(clear && accepted.folds.is_empty());
        assert_eq!(pipe.counters.losses.folds.load(Ordering::Relaxed), 1);
        if identity == cleared {
            assert!(accepted.waveform.is_none());
        } else {
            assert!(Arc::ptr_eq(&accepted.waveform.unwrap(), &waveform));
        }
    }
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
        assert!(pipe.accept(&mut picture, &mut true, &mut [0; 4], &mut (0.0, 0.0)).waveform.is_none());
        host::barrier(&pipe);
        let accepted = pipe.accept(&mut picture, &mut true, &mut [0; 4], &mut (0.0, 0.0));
        assert_eq!(accepted.frames, 0);
        if identity == current {
            assert!(Arc::ptr_eq(&accepted.waveform.unwrap(), &waveform));
        } else {
            assert!(accepted.waveform.is_none());
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
