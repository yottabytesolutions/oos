// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

/// STOP reads the stored record again for each view: zoomed in every sample of the part on screen (also far from
/// the trigger, which then lies outside the frame), zoomed out the whole record compressed; and the frame read
/// that way puts the trigger where the record had it.
#[test]
fn stop_reads_the_part_of_the_record_the_view_shows() {
    let m = Model {
        tscale: tscales().iter().position(|&t| (t as f64 - 1e-7).abs() < 1e-12).unwrap(),
        mdepth: mdepths().iter().position(|d| d.1 == 10_000_000).unwrap(),
        ..Default::default()
    };
    let s = settings(&m, false, &Default::default());
    let p = hz::prep(s.mask, oos_hal::dho::seq::tdiv_fs(s.tdiv), s.depth, oos_hal::dho::seq::tdiv_fs(s.delay));
    let b = batch_plan(&s, &p, 1);
    let rate = p.rate as f64;
    // as shown at the stop: a window around the trigger, every sample, the trigger inside
    let (r, ti) = stop_plan(&b, &p, 1e-7, 0.0);
    let (n, start) = oos_hal::dho::seq::window(&r, &p);
    assert_eq!((r.compress, r.frames), (1, 1));
    assert_eq!(ti, p.pre as f64 - start as f64);
    assert!(ti > 0.0 && ti < n as f64 && (n as f64) < 20_000.0, "{} {}", ti, n);
    // 1 us/div 3 ms after the trigger: the window there, the trigger far before it
    let (r, ti) = stop_plan(&b, &p, 1e-6, 3e-3);
    let (n, start) = oos_hal::dho::seq::window(&r, &p);
    assert_eq!(r.compress, 1);
    let centre = start as f64 + n as f64 / 2.0;
    assert!((centre - (p.pre as f64 + 3e-3 * rate)).abs() < 16.0, "{} {}", centre, p.pre);
    assert!((ti + 3e-3 * rate - n as f64 / 2.0).abs() < 16.0, "{}", ti);
    // 1 ms/div: the whole 8 ms record through the compression, the trigger at its point / k
    let (r, ti) = stop_plan(&b, &p, 1e-3, 0.0);
    assert!(r.compress > 1 && r.window.is_none());
    assert_eq!(ti, p.pre as f64 / r.compress as f64);
    // a frame of that read (a square with its edge at the trigger) becomes a record with the edge at 0 s
    let k = r.compress as usize;
    let len = (p.memsel as usize).div_ceil(k);
    let v: Vec<f32> = (0..len).map(|i| if (i as f64) < ti { -1.0 } else { 1.0 }).collect();
    let f = Frame {
        seq: 1,
        dt: k as f64 / rate,
        pre: 0,
        ch: [v, Vec::new(), Vec::new(), Vec::new()],
        envelope: true,
        acq: (rate, p.memsel as usize),
        fixed_ti: Some(ti),
        ..Default::default()
    };
    let rec = to_record(&Arc::new(f), &m, 3600, 10e-3, -5e-3);
    assert!(rec.triggered);
    let at = |t: f64| rec.ch[0][((t + 5e-3) / rec.dt) as usize];
    assert!(at(-1e-4) < 0.0 && at(1e-4) > 0.0, "{} {}", at(-1e-4), at(1e-4));
}
