// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

#[test]
fn excess_shares_combine_without_losing_pixel_weight_or_frame_counts() {
    for clear in [true, false] {
        let counters = Box::leak(Box::default());
        let pipe = Pipe::start(0, 0, counters);
        let model = Model::default();
        pipe.set_view(Some(&model), 16, 16, Interp::Exact);
        let (view, _, _) = pipe.view_now();
        let view = view.unwrap();
        let mail = &pipe.mails[0];
        for batch in 0..12 {
            let mut share = fresh_acc(None, &view);
            share.frames = 9;
            share.triggered = 7;
            share.edges = [9, 0, 0, 0];
            share.ph.map[0].fill(24 * 9);
            share.note(batch, None);
            publish(mail, share, counters);
            assert_eq!(oos_hal::sync::lock(mail).full.len(), 1, "one fixed map-sample budget per producer");
        }
        let mut ph = Phosphor::new(16, 16);
        ph.map[0].fill(5);
        let (mut edges, mut off) = ([0; 4], (0.0, 0.0));
        let mut clear_pending = clear;
        let got = pipe.merge(&mut ph, &mut clear_pending, &mut edges, &mut off);
        assert_eq!(got.frames, 108);
        assert_eq!(got.triggered, 84);
        assert_eq!(edges, [108, 0, 0, 0]);
        assert_eq!(ph.map[0], vec![108 * 24 + if clear { 0 } else { 5 }; 256]);
        assert_eq!(counters.read().2, 0);
    }
}

#[test]
fn accumulator_storage_follows_enabled_channels_and_releases_disabled_capacity() {
    let pipe = Pipe::start(0, 0, Box::leak(Box::default()));
    let mut model = Model::default();
    let mut spare = None;
    for channels in [4, 1, 2, 4, 1] {
        for (c, ch) in model.ch.iter_mut().enumerate() {
            ch.on = c < channels;
        }
        pipe.set_view(Some(&model), 970, 448, Interp::Exact);
        let (view, _, _) = pipe.view_now();
        let view = view.unwrap();
        let acc = fresh_acc(spare.take(), &view);
        assert_eq!(acc.ph.map.iter().map(Vec::len).sum::<usize>(), channels * 970 * 448);
        assert!(acc.ph.map.iter().skip(channels).all(|m| m.capacity() == 0));
        spare = Some(acc);
    }
}

#[test]
fn one_inflight_reduction_bounds_each_producer_to_three_accumulators() {
    let pipe = Pipe::start(0, 0, Box::leak(Box::default()));
    pipe.set_view(Some(&Model::default()), 16, 16, Interp::Exact);
    let view = pipe.view_now().0.unwrap();
    let mail = &pipe.mails[0];
    let (mut spare, mut allocations) = (None, 0);
    for _ in 0..100 {
        let mut held = None;
        for batch in 0..4 {
            if spare.is_none() {
                spare = oos_hal::sync::lock(mail).spare.pop();
            }
            allocations += usize::from(spare.is_none());
            let mut acc = fresh_acc(spare.take(), &view);
            acc.frames = 1;
            spare = publish(mail, acc, pipe.counters);
            if batch == 0 {
                held = oos_hal::sync::lock(mail).full.pop();
            }
        }
        oos_hal::sync::lock(mail).spare.extend(held);
        assert!(allocations <= 3, "one owned, one published, one reducing");
    }
}
