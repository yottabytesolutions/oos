// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

#[test]
fn scan_picture_wraps_and_fills_the_screen() {
    let mut pic = Vec::new();
    scan_write(&mut pic, 10, 0, &[1.0; 4]);
    scan_write(&mut pic, 10, 8, &[2.0; 4]); // wraps: 8, 9, 0, 1
    assert_eq!(&pic[..4], &[2.0, 2.0, 1.0, 1.0]);
    assert!(pic[4].is_nan() && pic[9] == 2.0);
    let mut ch: [Vec<f32>; NCH] = [pic, Vec::new(), Vec::new(), Vec::new()];
    scan_gap(&mut ch, 9, 2);
    assert!(ch[0][9].is_nan() && ch[0][0].is_nan() && ch[0][1] == 2.0);
    // a scan frame of 1000 points spans the screen: point 0 left, point 999 right
    let m = Model::default();
    let v: Vec<f32> = (0..1000).map(|i| i as f32).collect();
    let f = Frame {
        seq: 1,
        dt: 1e-3,
        pre: 0,
        ch: [v, Vec::new(), Vec::new(), Vec::new()],
        roll: false,
        envelope: true,
        scan: true,
        partial: true,
        acq: (0.0, 0),
        stamp: 0,
        la: None,
        fixed_ti: None,
        hw_avg: false,
        trigger_anchor: crate::board::TriggerAnchor::new(false),
        cfg: 0,
        basic: Default::default(),
    };
    let r = to_record(&Arc::new(f), &m, 100, 1.0, -0.5);
    assert!(r.partial && !r.triggered);
    assert!(r.ch[0][0] < 10.0 && r.ch[0][99] > 980.0, "{} {}", r.ch[0][0], r.ch[0][99]);
}

#[test]
fn scan_writes_the_la_where_the_analog_point_is() {
    // 25 000 analog points and 100 000 LA words over the screen
    assert_eq!(scan_la_pos(0, 25_000, 100_000), 0);
    assert_eq!(scan_la_pos(12_500, 25_000, 100_000), 50_000);
    assert_eq!(scan_la_pos(24_999, 25_000, 100_000), 99_996);
}

#[test]
fn short_time_bases_read_beyond_the_screen() {
    let mut m = Model {
        tscale: tscales().iter().position(|&x| (x as f64 - 1e-7).abs() < 1e-10).unwrap(),
        ..Default::default()
    };
    // 100 ns/div, one channel: 1250 points per screen at 1.25 GSa/s -> 1408 at the same rate
    let s = settings(&m, false, &Default::default());
    assert_eq!(s.depth, 1408);
    let p = hz::prep(s.mask, oos_hal::dho::seq::tdiv_fs(1e-7), s.depth, 0);
    assert_eq!((p.rate, p.memsel, p.pre + p.post), (1_250_000_000, 1408, 1408));
    assert_eq!(p.pre, 704);
    // the smallest records get at least 64 more points
    assert_eq!(screen_margin(62), 128);
    // 1 ns/div: 12 points per screen, read 80 at the full rate (more than stock's smallest record, 62 at 5 ns)
    m.tscale = tscales().iter().position(|&t| (t - 1e-9).abs() < 1e-12).unwrap();
    assert!((tscales()[m.tscale] - 1e-9).abs() < 1e-12);
    let s = settings(&m, false, &Default::default());
    let p = hz::prep(s.mask, oos_hal::dho::seq::tdiv_fs(1e-9), s.depth, 0);
    assert_eq!((p.rate, p.memsel, p.pre), (1_250_000_000, 80, 40), "depth {}", s.depth);
}
