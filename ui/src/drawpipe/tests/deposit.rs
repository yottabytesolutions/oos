// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

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
    assert_eq!(pipe.accept(&mut picture, &mut clear, &mut [0; 4], &mut (0.0, 0.0)).frames, 0);
    assert!(clear);
    assert_eq!(picture.map, visible);
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
