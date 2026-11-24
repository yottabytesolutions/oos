// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

#[test]
// Break it catches (docs/fork/display.md): Auto marking samples, or drawing dots, on a compressed record. The
// compression starts at 2 x POINTS_PER_COLUMN samples per column, Auto's marks at MARK_SPACING_PX pixels per sample.
fn auto_dots_never_meet_the_live_compression() {
    let mut seen = 0;
    for (t, &tdiv) in tscales().iter().enumerate() {
        for d in 0..mdepths().len() {
            for chans in [1usize, 2, 4] {
                let mut m = Model::default();
                assert_eq!(m.dots, DotsMode::Auto);
                (m.tscale, m.mdepth) = (t, d);
                for c in 0..NCH {
                    m.ch[c].on = c < chans;
                }
                let mut s = settings(&m, false, &Default::default());
                live_compress(&m, &mut s);
                if s.compress <= 1 {
                    continue;
                }
                seen += 1;
                let p = hz::prep(s.mask, oos_hal::dho::seq::tdiv_fs(s.tdiv), s.depth, 0);
                // the widest screen of the panel: pixels per sample of the raw record
                let x_per_sample = crate::layout::Screen::PANEL.w as f64 / HDIV as f64 / (p.rate as f64 * tdiv as f64);
                assert_eq!(
                    trace_style(m.dots, false, x_per_sample),
                    TraceStyle::Vectors,
                    "{} s/div depth {} {} ch: {} px per sample",
                    tdiv,
                    d,
                    chans,
                    x_per_sample
                );
                assert!(x_per_sample * (s.compress as f64) < 1.0, "{} s/div: a pair of k samples within a pixel", tdiv);
            }
        }
    }
    assert!(seen > 0, "some set-ups compress");
}

#[test]
fn live_compression_plan_by_time_base() {
    let plan = |tdiv: f64, two: bool| {
        let mut m = Model {
            dots: DotsMode::Off,
            tscale: tscales().iter().position(|&t| (t as f64 - tdiv).abs() < tdiv * 1e-6).unwrap(),
            ..Default::default()
        };
        // dots keep frames raw (compress::allowed)
        m.ch[1].on = two;
        let mut s = settings(&m, false, &Default::default());
        let raw = s.clone();
        live_compress(&m, &mut s);
        let p = hz::prep(s.mask, oos_hal::dho::seq::tdiv_fs(s.tdiv), s.depth, 0);
        let (b, r) = (batch_plan(&s, &p, 1), batch_plan(&raw, &p, 1));
        (b.compress, oos_hal::dho::seq::frame_bytes(&b, &p), b.frames, r.frames, p)
    };
    // every sample at 100 ns/div
    assert_eq!(plan(100e-9, false).0, 1);
    for (tdiv, two) in [(1e-6, false), (2e-6, false), (10e-6, false), (100e-6, false), (10e-6, true), (1e-3, false)] {
        let (k, fb, frames, raw_frames, p) = plan(tdiv, two);
        let per_column = p.rate as f64 * tdiv * 10.0 / crate::compress::SCREEN_COLUMNS;
        assert!(k >= crate::compress::K_MIN, "{} s/div: k {}", tdiv, k);
        // at least POINTS_PER_COLUMN points per column, less than twice that
        let points = per_column / k as f64;
        assert!(
            (crate::compress::POINTS_PER_COLUMN..2.0 * crate::compress::POINTS_PER_COLUMN).contains(&points),
            "{} s/div: {} points per column",
            tdiv,
            points
        );
        assert_eq!(
            fb,
            2 * p.ratio as usize
                * oos_hal::dho::seq::compressed_len(&Settings { compress: k, ..Settings::default() }, &p) as usize
        );
        assert!(frames >= raw_frames, "{} s/div: {} vs {} frames", tdiv, frames, raw_frames);
        assert!(raw_newest(&Settings { compress: k, ..Settings::default() }, &p));
    }
    // 10 us/div, one channel: 100 k points at 625 MSa/s (the Auto depth cap), 200 kB raw per frame become 13 kB,
    // 20 frames per batch become 300 (then the frame ring, 448, is the limit)
    let (k, fb, frames, raw_frames, p) = plan(10e-6, false);
    assert_eq!((p.rate, p.memsel, k, raw_frames), (625_000_000, 100_000, 15, 20), "{:?}", p);
    assert_eq!((fb, frames), (13_328, 300));
    // the model needs every sample
    let mut m = Model {
        tscale: tscales().iter().position(|&t| (t as f64 - 10e-6).abs() < 1e-11).unwrap(),
        trig_mode: TrigMode::Single,
        ..Default::default()
    };
    let mut s = settings(&m, false, &Default::default());
    live_compress(&m, &mut s);
    assert_eq!(s.compress, 1);
    // deep records keep the deep-memory read
    m.trig_mode = TrigMode::Auto;
    m.mdepth = mdepths().iter().position(|d| d.1 == 10_000_000).unwrap();
    let mut s = settings(&m, false, &Default::default());
    live_compress(&m, &mut s);
    assert_eq!(s.compress, 1);
}

// Break it catches: with compression the unit default, a mode that must see every sample (or the records as raw
// samples) getting min/max pairs. Every row must leave the frames raw; the base set-up must compress, so the table
// tests something. STOP: the record left on screen is a raw final frame (`ui::tests::
// live_pair_records_keep_the_raw_measurement_and_stop_takes_a_raw_frame`); measurements and cursors get raw
// records (`compress::hold`).
#[test]
fn modes_that_need_raw_frames_never_compress() {
    let base = || Model {
        tscale: tscales().iter().position(|&t| (t as f64 - 10e-6).abs() < 1e-11).unwrap(),
        dots: DotsMode::Off,
        ..Default::default()
    };
    let k = |m: &Model, needs_raw: bool| {
        let mut s = settings(m, false, &Default::default());
        live_compress(m, &mut s);
        raw_for_ui(&mut s, needs_raw);
        s.compress
    };
    assert!(k(&base(), false) > 1, "the base set-up compresses");
    for (name, f) in modes_that_use_raw_samples() {
        let mut m = base();
        f(&mut m);
        assert_eq!(k(&m, false), 1, "{}", name);
    }
    // the readers only the UI knows of (protocol watch, XY): `Shared::needs_raw`
    assert_eq!(k(&base(), true), 1, "protocol watch / XY");
    // the LA reads its own raw window of the record (only where the platform has one)
    let mut m = base();
    m.la.on = true;
    if m.la_on() {
        assert_eq!(k(&m, false), 1, "LA");
    }
}

type ModeCase = (&'static str, fn(&mut Model));

fn modes_that_use_raw_samples() -> &'static [ModeCase] {
    use crate::math::MathOp;
    &[
        ("dots", |m| m.dots = DotsMode::On),
        ("Single", |m| m.trig_mode = TrigMode::Single),
        ("zoom", |m| m.zoom = true),
        ("FFT math channel", |m| m.maths[0].op = MathOp::Fft),
        ("math channel", |m| m.maths[1].op = MathOp::Add),
        ("math (first page)", |m| m.math = MathOp::Sub),
        ("protocol decode", |m| m.dec_on = true),
        ("spectrum view", |m| m.view = AView::Spectrum),
        ("eye view", |m| m.view = AView::Eye),
        ("power view", |m| m.view = AView::Power),
        ("Bode view", |m| m.view = AView::Bode),
        ("step view", |m| m.view = AView::Step),
        ("mask", |m| m.mask_on = true),
        ("zone", |m| {
            m.zones[0] =
                crate::zone::Zone { on: true, src: 0, inside: true, pts: vec![(0.0, 0.0), (1e-6, 1.0), (2e-6, 0.0)] };
        }),
        ("Peak", |m| m.acq = AcqMode::Peak),
        ("Average", |m| m.acq = AcqMode::Average),
        ("High res", |m| m.acq = AcqMode::HiRes),
        ("channel AC", |m| m.ch[0].coupling = Coupling::Ac),
        ("20 MHz limit", |m| m.ch[0].bw = BW_20M),
        ("trigger AC", |m| m.trig_coup = TrigCoup::Ac),
        ("trigger LF reject", |m| m.trig_coup = TrigCoup::LfReject),
        ("trigger HF reject", |m| m.trig_coup = TrigCoup::HfReject),
        ("pulse trigger", |m| m.trig_type = TrigType::Pulse),
        ("segments", |m| m.segments = 8),
        ("roll", |m| {
            m.tscale = tscales().iter().position(|&t| t as f64 >= ROLL_TDIV).unwrap();
            m.auto_roll = true;
        }),
        ("scan", |m| {
            m.tscale = tscales().iter().position(|&t| t as f64 >= SCAN_TDIV).unwrap();
            m.auto_roll = false;
        }),
        ("Z input", |m| m.z_src = Some(1)),
    ]
}
