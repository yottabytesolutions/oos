// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

#[test]
fn watchdog_reset_releases_the_full_handoff_without_another_batch() {
    let m = model(1e-6, false, false);
    let pipe = Pipe::start(1, 0, fresh());
    pipe.set_view(Some(&m), 97, 48, Interp::Exact);
    let (bytes, fb, s, p) = batch(1, 400, false);
    for _ in 0..2 {
        pipe.feed(Box::new(Bytes(bytes.clone())), fb, 1, 0, dec(&s, &p));
        drop(recycled(&pipe));
    }
    host::barrier(&pipe);
    let gen = oos_hal::sync::lock(&pipe.view).gen;
    pipe.slot.recycle(Deposit::new(gen, (97, 48), None, [true; NCH]));

    shutdown::reset(&pipe);

    assert!(!pipe.accepting());
    assert!(pipe.slot.spare().is_none(), "reset releases recycled storage");
    crate::ui::lifecycle::test_wait::until("public and private deposits discarded", || {
        (pipe.counters.read().2 == 2).then_some(())
    });
    assert!(pipe.slot.take().is_none(), "the private deposit cannot reappear in the public slot");
}

#[test]
fn watchdog_reset_fences_in_flight_and_queued_batches_before_reopening() {
    let m = model(1e-6, false, false);
    let pipe = Pipe::start(2, 0, fresh());
    pipe.set_view(Some(&m), 97, 48, Interp::Exact);
    let (bytes, fb, s, p) = batch(1, 400, false);
    pipe.feed(Box::new(Bytes(bytes.clone())), fb, 1, 0, dec(&s, &p));
    drop(recycled(&pipe));
    let gate = one_worker_held(&pipe);
    for _ in 0..16 {
        pipe.feed(Box::new(Bytes(bytes.clone())), fb, 1, 0, dec(&s, &p));
    }

    shutdown::reset(&pipe);
    pipe.set_view(Some(&m), 97, 48, Interp::Exact);
    *gate.0.lock().unwrap() = true;
    gate.1.notify_all();
    settle(&pipe);
    host::barrier(&pipe);

    assert_eq!(pipe.counters.read().2, 37, "every pre-reset record is discarded once");
    assert!(pipe.slot.take().is_none());
    assert!(pipe.slot.spare().is_none());
    pipe.feed(Box::new(Bytes(bytes)), fb, 1, 0, dec(&s, &p));
    settle(&pipe);
    assert_eq!(pipe.accept(&mut Phosphor::new(97, 48), &mut true, &mut [0; 4], &mut (0.0, 0.0)).frames, 1);
}

#[test]
fn coalesced_batches_are_whole_and_flush_without_another_read() {
    let m = model(1e-6, false, false);
    let pipe = Pipe::start(4, 0, fresh());
    pipe.set_view(Some(&m), 97, 48, Interp::Exact);
    let (bytes, fb, s, p) = batch(70, 12_512, false);
    for _ in 0..3 {
        pipe.feed(Box::new(Bytes(bytes.clone())), fb, 70, 0, dec(&s, &p));
        drop(recycled(&pipe));
    }
    let mut picture = Phosphor::new(97, 48);
    let first = pipe.accept(&mut picture, &mut true, &mut [0; 4], &mut (0.0, 0.0));
    assert_eq!(first.frames, 70);
    host::barrier(&pipe);
    let second = pipe.accept(&mut picture, &mut false, &mut [0; 4], &mut (0.0, 0.0));
    assert_eq!(second.frames, 140);
    let (want, ..) = reference(&bytes.repeat(3), fb, &s, &p, &m, 97, 48);
    assert_eq!(picture.map, want.map);
    assert_eq!(pipe.counters.read().2, 0);
}

#[test]
fn completion_without_records_keeps_the_visible_picture() {
    let m = model(1e-6, false, false);
    let pipe = Pipe::start(2, 0, fresh());
    pipe.set_view(Some(&m), 97, 48, Interp::Exact);
    let (bytes, fb, s, p) = batch(3, 12_512, false);
    pipe.feed(Box::new(Bytes(bytes.clone())), fb, 3, 0, dec(&s, &p));
    drop(recycled(&pipe));
    let mut picture = Phosphor::new(97, 48);
    assert_eq!(pipe.accept(&mut picture, &mut true, &mut [0; 4], &mut (0.0, 0.0)).frames, 3);
    let visible = picture.map.clone();
    pipe.feed_batch(Box::new(Bytes(bytes)), fb, 0..0, (0, None), dec(&s, &p), None);
    drop(recycled(&pipe));
    let mut clear = true;
    let (mut edges, mut off) = ([0; 4], (0.0, 0.0));
    assert_eq!(pipe.accept(&mut picture, &mut clear, &mut edges, &mut off).frames, 0);
    assert!(clear);
    assert_eq!(picture.map, visible);
    assert_eq!((edges, off), ([0; 4], (0.0, 0.0)));
}

#[test]
fn deposits_close_before_the_fixed_weight_can_wrap() {
    let m = model(1e-7, false, false);
    let pipe = Pipe::start(2, 0, fresh());
    pipe.set_view(Some(&m), 17, 16, Interp::Exact);
    let (bytes, fb, s, p) = batch(1000, 400, false);
    for _ in 0..3 {
        pipe.feed(Box::new(Bytes(bytes.clone())), fb, 1000, 0, dec(&s, &p));
        drop(recycled(&pipe));
    }
    pipe.feed(Box::new(Bytes(bytes.clone())), fb, 1000, 0, dec(&s, &p));
    let mut picture = Phosphor::new(17, 16);
    assert_eq!(pipe.accept(&mut picture, &mut true, &mut [0; 4], &mut (0.0, 0.0)).frames, 1000);
    drop(recycled(&pipe));
    assert_eq!(pipe.accept(&mut picture, &mut false, &mut [0; 4], &mut (0.0, 0.0)).frames, 2000);
    host::barrier(&pipe);
    assert_eq!(pipe.accept(&mut picture, &mut false, &mut [0; 4], &mut (0.0, 0.0)).frames, 1000);
    let (want, ..) = reference(&bytes.repeat(4), fb, &s, &p, &m, 17, 16);
    assert_eq!(picture.map, want.map);
    assert!(picture.map[0].contains(&u16::MAX), "the control exercises saturated pixels");
    assert_eq!(pipe.counters.read().2, 0);
}
