// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

// Break it catches (docs/fork/stop-exact.md): a replay at STOP of a frame already raw, of a ring armed again, without
// the deep measurements, or of a batch that was not read through the compression
#[test]
fn final_replay_at_stop() {
    let mut m = Model {
        dots: DotsMode::Off,
        tscale: tscales().iter().position(|&t| (t as f64 - 1e-3).abs() < 1e-9).unwrap(),
        mdepth: mdepths().iter().position(|d| d.1 == 10_000_000).unwrap(),
        ..Default::default()
    };
    // dots keep frames raw (compress::allowed)
    let s = settings(&m, false, &Default::default());
    let p = hz::prep(s.mask, oos_hal::dho::seq::tdiv_fs(s.tdiv), s.depth, oos_hal::dho::seq::tdiv_fs(s.delay));
    let b = batch_plan(&s, &p, 1);
    let w = Some(crate::deepmeas::Want { every: false });
    assert_eq!(final_replay(&b, &p, 3, false, false, w), deep_raw_settings(&b, &p));
    assert!(final_replay(&b, &p, 3, false, false, w).is_some());
    assert!(final_replay(&b, &p, 3, true, false, w).is_none(), "newest frame already raw");
    assert!(final_replay(&b, &p, 3, false, true, w).is_none(), "armed ahead");
    assert!(final_replay(&b, &p, 3, false, false, None).is_none(), "no deep measurements");
    assert!(final_replay(&b, &p, 0, false, false, w).is_none());
    assert!(final_replay(&Settings { compress: 1, ..b.clone() }, &p, 3, false, false, w).is_none(), "read raw");
    // prearm on (the unit's set-up): no arming ahead for this batch, so the replay at STOP stays possible
    assert!(!prearm_keeps_replay(&b, &p, w));
    let ahead = prearm_keeps_replay(&b, &p, w);
    assert!(final_replay(&b, &p, 3, false, ahead, w).is_some());
    // prearm stays where no deep replay can come: without the deep measurements, a shallow record
    assert!(prearm_keeps_replay(&b, &p, None));
    m.mdepth = 0;
    m.tscale = tscales().iter().position(|&t| (t as f64 - 1e-6).abs() < 1e-12).unwrap();
    let s = settings(&m, false, &Default::default());
    let p = hz::prep(s.mask, oos_hal::dho::seq::tdiv_fs(s.tdiv), s.depth, 0);
    assert!(prearm_keeps_replay(&batch_plan(&s, &p, 1), &p, w));
    // the live compression at 10 us/div: the whole newest frame raw at STOP, with or without deep measurements;
    // no arming ahead for its batches
    m.tscale = tscales().iter().position(|&t| (t as f64 - 10e-6).abs() < 1e-11).unwrap();
    let mut s = settings(&m, false, &Default::default());
    live_compress(&m, &mut s);
    let p = hz::prep(s.mask, oos_hal::dho::seq::tdiv_fs(s.tdiv), s.depth, 0);
    let b = batch_plan(&s, &p, 1);
    assert!(b.compress > 1 && raw_newest(&b, &p));
    let r = final_replay(&b, &p, 3, false, false, None).expect("raw newest at STOP");
    assert_eq!((r.compress, r.window), (1, None));
    assert!(final_replay(&b, &p, 3, true, false, None).is_none(), "newest frame already raw");
    assert!(!prearm_keeps_replay(&b, &p, None));
}

// Break it catches: a raw deep read of the wrong part of the record (screen or trigger point outside it), one
// beyond the budget, one where the batch was already raw or of the live compression, or in Peak.
#[test]
fn deep_raw_window_for_the_measurements() {
    let plan = |tdiv: f64, depth: u64, two: bool| {
        let mut m = Model {
            tscale: tscales().iter().position(|&t| (t as f64 - tdiv).abs() < tdiv * 1e-6).unwrap(),
            mdepth: mdepths().iter().position(|d| d.1 == depth).unwrap(),
            ..Default::default()
        };
        m.ch[1].on = two;
        let s = settings(&m, false, &Default::default());
        let p = hz::prep(s.mask, oos_hal::dho::seq::tdiv_fs(s.tdiv), s.depth, oos_hal::dho::seq::tdiv_fs(s.delay));
        let b = batch_plan(&s, &p, 1);
        (b.clone(), p, deep_raw_settings(&b, &p))
    };
    // 10 M at 1 ms/div, CH1: the batch reads pairs, the measurement window raw: the whole record, 20 MB
    let (b, p, d) = plan(1e-3, 10_000_000, false);
    assert!(b.compress > 1 && b.window.is_none());
    let d = d.expect("raw window");
    assert_eq!(d.compress, 1);
    let (n, start) = oos_hal::dho::seq::window(&d, &p);
    let screen = (b.tdiv * 10.0 * p.rate as f64) as u32;
    assert!(
        start <= (p.memsel / 2).saturating_sub(screen / 2) && start + n >= (p.memsel / 2 + screen / 2).min(p.memsel),
        "{} + {} of {}",
        start,
        n,
        p.memsel
    );
    assert!(start < p.pre && start + n > p.pre);
    let bytes = oos_hal::dho::seq::frame_bytes(&d, &p);
    assert!(bytes <= crate::deepmeas::MAX_BYTES, "{} bytes", bytes);
    // the decoded frame is raw and knows its trigger point
    let f = decode(&vec![0u8; bytes], &d, &p, 1);
    assert!(!f.envelope && f.ch[0].len() == n as usize && f.pre == (p.pre - start) as usize);
    // 2 channels at 10 M: the quarter-screen margins made 37.5 MB; narrower margins fit the budget
    let (_, p2, d2) = plan(1e-3, 10_000_000, true);
    assert!(oos_hal::dho::seq::frame_bytes(&d2.expect("narrow window"), &p2) <= crate::deepmeas::MAX_BYTES);
    // 25 M at 2 ms/div: 50 MB
    assert!(plan(2e-3, 25_000_000, false).2.is_none());
    // a deep record at a short time base is read raw already (screen window), 1 M is the live path
    assert!(plan(1e-7, 10_000_000, false).2.is_none());
    assert!(plan(1e-3, 1_000_000, false).2.is_none());
    // Peak: the memory holds peak pairs
    let (b, p, _) = plan(1e-3, 10_000_000, false);
    assert!(deep_raw_settings(&Settings { mode: oos_hal::dho::acq::Mode::Peak, ..b }, &p).is_none());
}

#[test]
fn a_deep_record_at_a_short_time_base_reads_the_screen_window() {
    // 10 M at 100 ns/div, CH1: 1.25 GSa/s, the screen is 1250 of the 10 M points
    let mut m = Model {
        tscale: tscales().iter().position(|&t| (t as f64 - 1e-7).abs() < 1e-12).unwrap(),
        mdepth: mdepths().iter().position(|d| d.1 == 10_000_000).unwrap(),
        ..Default::default()
    };
    let s = settings(&m, false, &Default::default());
    let p = hz::prep(s.mask, oos_hal::dho::seq::tdiv_fs(s.tdiv), s.depth, oos_hal::dho::seq::tdiv_fs(s.delay));
    let b = batch_plan(&s, &p, 1);
    assert_eq!(b.compress, 1, "every sample, not ~80 min/max points for the screen");
    let (n, start) = oos_hal::dho::seq::window(&b, &p);
    assert!(n < 20_000 && n % 8 == 0, "{}", n);
    // the screen and the trigger point are inside, and the frame's trigger point counts from the window
    let screen = (s.tdiv * 10.0 * p.rate as f64) as u32;
    assert!(start <= p.memsel / 2 - screen / 2 && start + n >= p.memsel / 2 + screen / 2);
    assert!(start < p.pre && start + n > p.pre);
    let bytes = vec![0u8; seq_bytes(&b, &p)];
    let f = decode(&bytes, &b, &p, 1);
    assert_eq!(f.ch[0].len(), n as usize);
    assert_eq!(f.pre, (p.pre - start) as usize);
    // a long time base keeps the compressed whole record
    m.tscale = tscales().iter().position(|&t| (t as f64 - 1e-3).abs() < 1e-9).unwrap();
    let s = settings(&m, false, &Default::default());
    let p = hz::prep(s.mask, oos_hal::dho::seq::tdiv_fs(s.tdiv), s.depth, 0);
    let b = batch_plan(&s, &p, 1);
    assert!(b.compress > 1 && b.window.is_none());
}
