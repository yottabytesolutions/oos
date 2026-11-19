// SPDX-License-Identifier: GPL-2.0-or-later

use super::kernel::{self, SamplesRef};
use super::standard;
use crate::fastdraw::{Codes, Samples};
use crate::model::*;

#[test]
fn codes_and_volts_place_and_deposit_identically() {
    for channels in [1, 2, 4] {
        for dots in [false, true] {
            for scale in [0.01, -0.01] {
                let mut m = Model { dots: if dots { DotsMode::On } else { DotsMode::Off }, ..Default::default() };
                for (c, ch) in m.ch.iter_mut().enumerate() {
                    ch.on = c < channels;
                    ch.offset = c as f32 * 0.2;
                }
                let words: Vec<u16> = (0..4096 * channels).map(|i| ((i * 37 + i / 101) % 1024) as u16).collect();
                let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
                let codes = |c| Codes {
                    bytes: &bytes,
                    words: Some(&words),
                    stride: channels,
                    word: c,
                    n: 4096,
                    affine: (scale, 5.0),
                    volt: |x, (scale, offset)| x as f32 * scale - offset,
                };
                let volts: [Vec<f32>; NCH] = std::array::from_fn(|c| {
                    if c < channels {
                        (0..4096).map(|i| codes(c).at(i)).collect()
                    } else {
                        Vec::new()
                    }
                });
                let frame = crate::board::Frame {
                    dt: m.time_per_div() as f64 * HDIV as f64 / 4096.0,
                    pre: 2048,
                    ..Default::default()
                };
                let meta = standard::frame_timing(&m, &frame);
                let proj = standard::projection(&m, 97, 48, 97.0 / 4096.0);
                let raw = kernel::place(
                    std::array::from_fn(|c| {
                        if c < channels {
                            SamplesRef::Codes(codes(c))
                        } else {
                            SamplesRef::Volts(&[])
                        }
                    }),
                    &meta,
                    &proj,
                    0.0,
                    [true; NCH],
                );
                let decoded_samples = std::array::from_fn(|c| SamplesRef::Volts(&volts[c]));
                let decoded = kernel::place(decoded_samples, &meta, &proj, 0.0, [true; NCH]);
                assert_eq!(raw.trace.trigger_at, decoded.trace.trigger_at);
                assert_eq!(raw.trace.outcome, decoded.trace.outcome);
                compare_planes(
                    &proj,
                    &raw,
                    &decoded,
                    std::array::from_fn(|c| (c < channels).then(|| codes(c))),
                    &volts,
                );
            }
        }
    }
}

fn compare_planes(
    proj: &kernel::Projection,
    raw: &kernel::Placed,
    decoded: &kernel::Placed,
    codes: [Option<Codes<'_>>; NCH],
    volts: &[Vec<f32>; NCH],
) {
    for c in 0..NCH {
        assert_eq!(
            raw.fold[c].map(|f| (f.n, f.min, f.max, f.sum, f.sumsq)),
            decoded.fold[c].map(|f| (f.n, f.min, f.max, f.sum, f.sumsq))
        );
        let mut a = vec![0u16; proj.cols * (proj.rows + 1)];
        let mut b = a.clone();
        if let Some(codes) = codes[c] {
            kernel::add_placed_band(c, &mut a, 0, &SamplesRef::Codes(codes), &raw.projection, &raw.trace);
        } else {
            assert!(raw.trace.spans[c].is_none() && raw.fold[c].is_none());
        }
        kernel::add_placed_band(c, &mut b, 0, &SamplesRef::Volts(&volts[c]), &decoded.projection, &decoded.trace);
        assert_eq!(a, b, "channel={c}");
    }
}

#[test]
fn placement_uses_captured_inputs_after_settings_change() {
    let mut m = Model::default();
    let samples: Vec<f32> = (0..4096).map(|i| (i as f32 * 0.03).sin()).collect();
    let f = crate::board::Frame { dt: 1e-8, pre: 2048, ..Default::default() };
    let meta = standard::frame_timing(&m, &f);
    let proj = standard::projection(&m, 97, 48, 0.01);
    let place = || kernel::place(std::array::from_fn(|_| SamplesRef::Volts(&samples)), &meta, &proj, 0.0, [true; NCH]);
    let before = place();
    m.delay = 1.0;
    m.ch[0].offset = 100.0;
    m.ch[0].on = false;
    m.trig_level = 100.0;
    let after = place();
    assert_eq!(before.trace.trigger_at, after.trace.trigger_at);
    assert_eq!(before.trace.outcome, after.trace.outcome);
    assert_eq!(before.trace.spans[0].as_ref().unwrap().rows, after.trace.spans[0].as_ref().unwrap().rows);
    assert_ne!(standard::projection(&m, 97, 48, 0.01).delay, proj.delay);
}

#[test]
fn raw_plan_reuses_placed_projection_for_every_band() {
    let (w, h) = (97, 48);
    for dots in [DotsMode::On, DotsMode::Off] {
        let m = Model { dots, ..Default::default() };
        let f = std::sync::Arc::new(crate::board::Frame {
            dt: m.time_per_div() as f64 * HDIV as f64 / 4096.0,
            pre: 2048,
            ch: std::array::from_fn(|_| (0..4096).map(|i| (i as f32 * 0.03).sin()).collect()),
            ..Default::default()
        });
        let frame = standard::fast_plan(&m, w, h, &f, [false; NCH]);
        assert!(frame.plan.outcome().is_some());
        let mut reference = vec![0; w * h];
        standard::fast_deposit(0, &mut reference, 0, &f, &frame.plan);
        assert!(reference.iter().any(|&v| v != 0));
        let mut columns = vec![0; w * (h + 1)];
        let band_cols = 13;
        for (b, band) in columns.chunks_mut(band_cols * (h + 1)).enumerate() {
            standard::add_frame_band(0, band, b * band_cols, &f, &frame.plan);
        }
        let mut band_map = vec![0; w * h];
        crate::fastdraw::fold_t(&mut band_map, &mut columns, w, h, band_cols, 0..h);
        assert_eq!(band_map, reference);
    }
}

#[test]
fn pair_placement_reuses_and_clears_span_storage() {
    let (w, h) = (97, 48);
    let mut m = Model { dots: DotsMode::Off, ..Default::default() };
    m.ch[0].on = true;
    let mut f = crate::board::Frame {
        dt: m.time_per_div() as f64 * HDIV as f64 / 512.0,
        pre: 256,
        ch: [vec![0.0; 512], Vec::new(), Vec::new(), Vec::new()],
        envelope: true,
        ..Default::default()
    };
    let first = standard::batch_plan(&m, (w, h), &f, None, None).plan.into_raw().unwrap();
    let spans = first.spans[0].as_ref().unwrap();
    assert!(spans.rows.iter().all(|&(lo, hi)| lo <= hi));
    let storage = spans.rows.as_ptr();

    f.ch[0].truncate(2);
    f.pre = 1;
    let fresh = standard::batch_plan(&m, (w, h), &f, None, None).plan.into_raw().unwrap();
    let reused = standard::batch_plan(&m, (w, h), &f, None, Some(first)).plan.into_raw().unwrap();
    let (fresh, reused) = (fresh.spans[0].as_ref().unwrap(), reused.spans[0].as_ref().unwrap());
    assert!(fresh.rows.iter().any(|&(lo, hi)| lo > hi));
    assert_eq!((reused.c0, &reused.rows), (fresh.c0, &fresh.rows));
    assert_eq!(reused.rows.as_ptr(), storage);
}

#[test]
fn projection_dimensions_do_not_truncate_at_u16_boundary() {
    for (w, h, dots) in [(65_537, 8, false), (65_537, 8, true), (8, 65_537, true)] {
        let mut m = Model { dots: if dots { DotsMode::On } else { DotsMode::Off }, ..Default::default() };
        m.ch[0].offset = 0.0;
        let proj = standard::projection(&m, w, h, 0.25);
        assert_eq!((proj.cols, proj.rows), (w, h));
        let f = crate::board::Frame { dt: 1e-8, ..Default::default() };
        let mut meta = standard::frame_timing(&m, &f);
        meta.trigger.on_la = true;
        let samples = [0.0; 8];
        let placed = kernel::place(
            std::array::from_fn(|c| SamplesRef::Volts(if c == 0 { &samples } else { &[] })),
            &meta,
            &proj,
            0.0,
            [false; NCH],
        );
        let map = kernel::raw_map(&proj, 0, 0.0).unwrap();
        assert_eq!((map.x0, map.y0), (w as f64 / 2.0, h as f32 / 2.0));
        let mut expected = vec![0; w * h];
        if dots {
            let _ = crate::fastdraw::deposit_dots(&mut expected, w, h, &samples, &map, 24);
        } else {
            let _ = crate::fastdraw::deposit(&mut expected, w, h, &samples, &map, 24);
        }
        assert!(expected.iter().any(|&v| v != 0));
        let mut columns = vec![0; w * (h + 1)];
        kernel::add_placed_band(0, &mut columns, 0, &SamplesRef::Volts(&samples), &proj, &placed.trace);
        let mut actual = vec![0; w * h];
        crate::fastdraw::fold_t(&mut actual, &mut columns, w, h, w, 0..h);
        assert_eq!(actual, expected, "{w}x{h}, dots={dots}");
    }
}

#[test]
fn record_plan_bands_keep_the_placed_mapping_for_every_fallback() {
    let (w, h) = (97, 48);
    for route in 0..4 {
        for dots in [DotsMode::On, DotsMode::Off, DotsMode::Auto] {
            let mut m = Model { dots, ..Default::default() };
            m.ch[0].offset = 0.2;
            m.ch[1].on = false;
            m.ch[2].on = true;
            let values: Vec<f32> = (0..256).map(|i| (i as f32 * 0.1).sin()).collect();
            let f = std::sync::Arc::new(crate::board::Frame {
                dt: m.time_per_div() as f64 * HDIV as f64 / if route == 0 { 128.0 } else { 1024.0 },
                pre: 128,
                ch: [values.clone(), values, Vec::new(), Vec::new()],
                envelope: route == 1,
                roll: route == 2,
                scan: route == 3,
                ..Default::default()
            });
            let frame = standard::fast_plan(&m, w, h, &f, [true; NCH]);
            assert!(frame.plan.outcome().is_none());
            assert!(frame.plan.fold(0).is_some());
            assert!(frame.plan.fold(1).is_none());
            let window = m.time_per_div() as f64 * HDIV as f64;
            let rec = crate::board::to_record(&f, &m, 3600, window, m.delay as f64 - window / 2.0);
            let mut expected = crate::wave::Phosphor::new(w, h);
            expected.draw_ex(0, &m, (m.time_per_div(), m.delay), rec.t0, rec.dt, &rec.ch[0], 24, None, rec.pairs);
            assert!(expected.maps()[0].iter().any(|&weight| weight > 0));
            m.delay += m.time_per_div();
            m.ch[0].offset += 2.0 * m.ch[0].volts_per_div();
            m.dots = if dots == DotsMode::On { DotsMode::Off } else { DotsMode::On };
            let mut remapped = crate::wave::Phosphor::new(w, h);
            remapped.draw_ex(0, &m, (m.time_per_div(), m.delay), rec.t0, rec.dt, &rec.ch[0], 24, None, rec.pairs);
            assert!(remapped.maps()[0] != expected.maps()[0]);
            check_record_bands(&f, &frame.plan, &expected);
        }
    }
}

fn check_record_bands(f: &crate::board::Frame, plan: &standard::FastPlan, expected: &crate::wave::Phosphor) {
    let (w, h) = (expected.w, expected.h);
    for c in 0..3 {
        let mut row_map = vec![0; w * h];
        standard::fast_deposit(c, &mut row_map, 0, f, plan);
        assert_eq!(row_map, expected.maps()[c]);
        let mut partial_rows = vec![0; 17 * w];
        standard::fast_deposit(c, &mut partial_rows, 3, f, plan);
        assert_eq!(partial_rows, expected.maps()[c][3 * w..20 * w]);
        let mut columns = vec![0; w * (h + 1)];
        for (b, band) in columns.chunks_mut(13 * (h + 1)).enumerate() {
            standard::add_frame_band(c, band, b * 13, f, plan);
        }
        let mut actual = vec![0; w * h];
        let (lo, hi) = plan.rows(c);
        crate::fastdraw::fold_t(&mut actual, &mut columns, w, h, 13, lo..hi);
        assert_eq!(actual, expected.maps()[c], "channel={c}");
    }
}
