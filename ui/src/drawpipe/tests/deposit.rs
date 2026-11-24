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
    pipe.feed(Box::new(Bytes(bytes.clone())), fb, 1, 0, dec(&s, &p));

    shutdown::reset(&pipe);
    pipe.set_view(Some(&m), 97, 48, Interp::Exact);
    *gate.0.lock().unwrap() = true;
    gate.1.notify_all();
    settle(&pipe);
    host::barrier(&pipe);

    assert_eq!(pipe.counters.read().2, 22, "every pre-reset record is discarded once");
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
    let identity = Default::default();
    pipe.publish_view(&m, 97, 48, Interp::Exact, identity);
    let (bytes, fb, s, p) = batch(70, 12_512, false);
    let decode = dec(&s, &p);
    for seq in 1..=3 {
        let mut waveform = Frame::default();
        decode(&bytes[69 * fb..], &mut waveform);
        waveform.seq = seq;
        pipe.feed_with(
            Box::new(Bytes(bytes.clone())),
            fb,
            0..70,
            identity,
            decode.clone(),
            None,
            Some(Arc::new(waveform)),
        );
        drop(recycled(&pipe));
    }
    for seq in 1..=3 {
        let tap = pipe.take_waveform((97, 48)).unwrap();
        assert_eq!(tap.seq, seq);
        assert_eq!(tap.eligibility(), PictureEligibility::Pending);
    }
    assert!(pipe.take_waveform((97, 48)).is_none());
    let mut picture = Phosphor::new(97, 48);
    let first = pipe.accept(&mut picture, &mut true, &mut [0; 4], &mut (0.0, 0.0));
    assert_eq!(first.frames, 70);
    host::barrier(&pipe);
    let second = pipe.accept(&mut picture, &mut false, &mut [0; 4], &mut (0.0, 0.0));
    assert_eq!(second.frames, 140);
    let (want, ..) = reference(&bytes.repeat(3), fb, &s, &p, &m, 97, 48);
    assert_eq!(picture.maps(), want.maps());
    assert_eq!(pipe.counters.read().2, 0);
}

#[test]
fn draining_one_tap_releases_a_full_fifo_while_the_picture_slot_stays_full() {
    let m = model(1e-6, false, false);
    let pipe = Pipe::start(1, 0, fresh());
    let identity = Default::default();
    let size = (97, 48);
    pipe.publish_view(&m, size.0, size.1, Interp::Exact, identity);
    let (bytes, fb, s, p) = batch(1, 400, false);
    let decode = dec(&s, &p);
    let submit = |seq| {
        let mut waveform = Frame::default();
        decode(&bytes, &mut waveform);
        waveform.seq = seq;
        assert!(pipe
            .feed_with(
                Box::new(Bytes(bytes.clone())),
                fb,
                0..1,
                identity,
                decode.clone(),
                None,
                Some(Arc::new(waveform))
            )
            .is_none());
    };
    let capacity = crate::board::QUEUE_MAX as u64;
    for seq in 1..=capacity {
        submit(seq);
        drop(recycled(&pipe));
    }
    submit(capacity + 1);
    pipe.slot.wait_for_sender();
    assert_eq!(pipe.tap_count() as u64, capacity);
    assert_eq!(pipe.take_waveform(size).unwrap().seq, 1);
    drop(recycled(&pipe));
    assert_eq!(pipe.take_waveform(size).unwrap().seq, 2);
    assert!(pipe.slot.take().is_some(), "draining taps does not claim the picture slot");
    shutdown::reset(&pipe);
    assert_eq!(pipe.tap_count(), 0, "reset releases queued waveform owners");
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
    let visible = picture.maps().clone();
    pipe.feed_batch(Box::new(Bytes(bytes)), fb, 0..0, (0, None), dec(&s, &p), None);
    drop(recycled(&pipe));
    let mut clear = true;
    let (mut edges, mut off) = ([0; 4], (0.0, 0.0));
    assert_eq!(pipe.accept(&mut picture, &mut clear, &mut edges, &mut off).frames, 0);
    assert!(clear);
    assert_eq!(picture.maps(), &visible);
    assert_eq!((edges, off), ([0; 4], (0.0, 0.0)));
}

#[test]
fn sparse_batch_keeps_trigger_status_when_no_channel_is_shown() {
    let mut m = model(1e-7, false, false);
    m.ch[0].on = false;
    m.trig_coup = TrigCoup::Ac;
    let pipe = Pipe::start(1, 0, fresh());
    let (w, h) = (970, 448);
    pipe.set_view(Some(&m), w, h, Interp::Exact);
    let (bytes, fb, s, p) = batch(1, 1408, false);
    pipe.feed(Box::new(Bytes(bytes)), fb, 1, 0, dec(&s, &p));
    drop(recycled(&pipe));
    let mut picture = Phosphor::new(w, h);
    let accepted = pipe.accept(&mut picture, &mut true, &mut [0; 4], &mut (0.0, 0.0));
    assert_eq!((accepted.frames, accepted.triggered), (1, 1));
    assert!(picture.maps().iter().flatten().all(|&counts| counts == 0));
    assert_eq!(pipe.counters.read().2, 0);
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
    assert_eq!(picture.maps(), want.maps());
    assert!(picture.maps()[0].contains(&u16::MAX), "the control exercises saturated pixels");
    assert_eq!(pipe.counters.read().2, 0);
}

#[test]
fn full_slot_closes_a_dot_deposit_before_the_first_overflowing_batch() {
    full_dot_slot(&[(2, 15), (3, 16)], 1, false);
    full_dot_slot(&[(2, 16)], 16, false);
}

#[test]
fn reset_during_capacity_handoff_accounts_the_incoming_whole_batch() {
    full_dot_slot(&[(2, 15), (3, 16)], 1, true);
}

fn full_dot_slot(fitting: &[(u8, usize)], overflowing: usize, reset: bool) {
    let m = Model { dots: DotsMode::On, ..model(1e-6, false, false) };
    let pipe = Pipe::start(2, 0, fresh());
    let (w, h) = (4, 16);
    pipe.set_view(Some(&m), w, h, Interp::Exact);
    let dt = m.time_per_div() as f64 * HDIV as f64 / (w as f64 * 85.75);
    let decode: DecodeInto = Arc::new(move |bytes, f| {
        f.seq = u64::from(bytes[0]);
        f.dt = dt;
        f.pre = 172;
        f.ch[0].resize(344, 0.0);
    });
    let submit = |id, records| {
        Counters::add(&pipe.counters.acquired, records as u64);
        pipe.feed(Box::new(Bytes(vec![id; records])), 1, records, 0, decode.clone());
    };
    let returned = |id, records| {
        let raw = host::finish_batch(&pipe, u64::from(id)).unwrap();
        assert_eq!(raw.bytes(), &[id; 32][..records]);
    };
    // Fill the slot, then finish the largest fitting run of 86-hit records in the open plane.
    submit(1, 1);
    returned(1, 1);
    for &(id, records) in fitting {
        submit(id, records);
        returned(id, records);
    }
    submit(4, overflowing);
    pipe.slot.wait_for_sender();
    assert!(pipe.take_buffer().is_none(), "the blocked whole batch still owns its input");
    if reset {
        shutdown::reset(&pipe);
        returned(4, overflowing);
        host::barrier(&pipe);
        let counts = pipe.counters.records_text();
        assert_eq!(pipe.counters.read(), (33, 0, 33));
        assert!(counts.contains("read=33 deposited=32 shown=0"));
        assert!(counts.contains("lost.unclaimed=1"));
        assert!(counts.contains("rejected.stale_key=32"));
        assert!(pipe.slot.take().is_none());
        pipe.set_view(Some(&m), w, h, Interp::Exact);
        submit(5, 1);
        returned(5, 1);
        let got = pipe.accept(&mut Phosphor::new(w, h), &mut true, &mut [0; 4], &mut (0.0, 0.0));
        assert_eq!(got.frames, 1);
        return;
    }
    let mut picture = Phosphor::new(w, h);
    let mut accept = || {
        let d = pipe.accept(&mut picture, &mut true, &mut [0; 4], &mut (0.0, 0.0));
        (d.frames, picture.maps()[0].iter().copied().max().unwrap())
    };
    assert_eq!(accept(), (1, 86 * 24));
    returned(4, overflowing);
    assert!(pipe.slot.spare().is_none(), "the next batch reuses the first deposit's planes");
    let fitted: u16 = fitting.iter().map(|(_, records)| *records as u16).sum();
    assert_eq!(accept(), (u64::from(fitted), fitted * 86 * 24));
    host::barrier(&pipe);
    assert_eq!(accept(), (overflowing as u64, overflowing as u16 * 86 * 24));
    assert_eq!(accept().0, 0);
    assert_eq!(pipe.counters.read().2, 0);
}

#[test]
fn single_oversized_dot_submission_returns_its_slot_and_accounts_every_record() {
    let m = Model { dots: DotsMode::On, ..model(1e-6, false, false) };
    let pipe = Pipe::start(1, 0, fresh());
    pipe.set_view(Some(&m), 4, 16, Interp::Exact);
    let dt = m.time_per_div() as f64 * HDIV as f64 / (4.0 * 85.75);
    let decode: DecodeInto = Arc::new(move |_, f| {
        f.dt = dt;
        f.pre = 172;
        f.ch[0].resize(344, 0.0);
    });
    Counters::add(&pipe.counters.acquired, 32);
    pipe.feed(Box::new(Bytes(vec![0; 32])), 1, 32, 0, decode.clone());
    assert_eq!(recycled(&pipe).bytes(), &[0; 32]);
    let counts = pipe.counters.records_text();
    assert!(counts.contains("read=32 deposited=0 shown=0"));
    assert!(counts.contains("lost.budget=32"));
    assert!(counts.contains("lost.unclaimed=0"));
    assert_eq!(pipe.counters.read(), (32, 0, 32));
    assert!(pipe.slot.take().is_none());
    Counters::add(&pipe.counters.acquired, 1);
    pipe.feed(Box::new(Bytes(vec![0])), 1, 1, 0, decode);
    drop(recycled(&pipe));
    let got = pipe.accept(&mut Phosphor::new(4, 16), &mut true, &mut [0; 4], &mut (0.0, 0.0));
    assert_eq!(got.frames, 1, "an unsupported submission cannot kill the worker");
    assert!(pipe.counters.records_text().contains("read=33 deposited=1 shown=0"));
}

#[test]
fn whole_deposits_publish_changed_rows_to_independent_gpu_consumers() {
    for channels in [1, 2, 4] {
        for dots in [false, true] {
            for tdiv in [1e-6, 1e-8] {
                check_upload_rows(channels, dots, tdiv);
            }
        }
    }
}

fn check_upload_rows(channels: usize, dots: bool, tdiv: f64) {
    let (w, h) = (97, 48);
    let mut m = model(tdiv, false, dots);
    for (c, channel) in m.ch.iter_mut().enumerate() {
        channel.on = c < channels;
    }
    let pipe = Pipe::start(4, 0, fresh());
    pipe.set_view(Some(&m), w, h, Interp::Exact);
    let mut picture = Phosphor::new(w, h);
    let initial: [_; NCH] = std::array::from_fn(|c| picture.upload_rows(c, None).0);
    let mut panel = initial.clone();
    let mut panel_maps = picture.maps().clone();
    let mut monitor_maps = panel_maps.clone();
    for (step, level) in [-0.5, 0.5].into_iter().enumerate() {
        let (bytes, fb, s, p) = frames_of((1 << channels) - 1, 12_512, 3, &|_, c, _| level + c as f64 * 0.1);
        pipe.feed(Box::new(Bytes(bytes)), fb, 3, 0, dec(&s, &p));
        drop(recycled(&pipe));
        let accepted = pipe.accept(&mut picture, &mut true, &mut [0; 4], &mut (0.0, 0.0));
        assert_eq!(accepted.frames, 3);
        for c in 0..NCH {
            let (stamp, rows) = picture.upload_rows(c, Some(&panel[c]));
            if c >= channels {
                assert!(rows.is_empty(), "disabled channels need no upload");
                continue;
            }
            assert!(!rows.is_empty(), "channel {c}, dots={dots}, step={step}");
            assert!(!picture.is_wide(), "this fixture must exercise ordinary narrow storage");
            assert!(rows.len() < h, "flat traces retain a narrow upload band, dots={dots}");
            let range = rows.start * w..rows.end * w;
            panel_maps[c][range.clone()].copy_from_slice(&picture.maps()[c][range]);
            panel[c] = stamp;
            if step == 1 {
                let (_, rows) = picture.upload_rows(c, Some(&initial[c]));
                let range = rows.start * w..rows.end * w;
                monitor_maps[c][range.clone()].copy_from_slice(&picture.maps()[c][range]);
            }
        }
        assert_eq!(&panel_maps, picture.maps(), "cleared and newly deposited rows must both reach the panel");
    }
    assert_eq!(&monitor_maps, picture.maps(), "a skipped monitor frame retains every intervening row change");
    assert_eq!(pipe.accept(&mut picture, &mut true, &mut [0; 4], &mut (0.0, 0.0)).frames, 0);
    for (c, stamp) in panel.iter().enumerate() {
        assert!(picture.upload_rows(c, Some(stamp)).1.is_empty(), "unchanged pictures need no upload");
    }
    assert_eq!(pipe.counters.deposited.load(Ordering::Relaxed), 6);
    assert_eq!(pipe.counters.read().2, 0);
}
