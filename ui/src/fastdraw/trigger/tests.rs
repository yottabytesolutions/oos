// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

#[test]
fn edge_is_refined_near_pre() {
    // a ramp crossing 0.5 between samples 11 and 12 (at 11.25), another rising crossing far away
    let mut v: Vec<f32> = (0..40).map(|k| (k as f32 - 9.0) * 0.4 - 0.4).collect();
    v[30] = -1.0;
    v[31] = 1.0;
    let t = edge_near(&v, 11, 0.5, 0.0, true, false, 4).unwrap();
    assert!((t - 11.25).abs() < 1e-4, "{}", t);
    assert!(edge_near(&v, 11, 0.5, 0.0, false, false, 4).is_none());
    // the nearest of two crossings wins
    assert!((edge_near(&v, 29, 0.0, 0.0, true, false, 40).unwrap() - 30.5).abs() < 1e-4);
}

/// A slow sine with noise larger than the band, the hardware point on the falling edge: the smoothed search finds
/// the rising edge half a period away, never the noise on the falling one.
#[test]
fn smoothed_search_takes_the_trend_not_the_noise() {
    // 12500 samples a period, amplitude 0.55, noise +-0.075 (pseudo random), level 0, band 0.06
    let mut r = 12345u32;
    let v: Vec<f32> = (0..50_000)
        .map(|k| {
            r = r.wrapping_mul(1_103_515_245).wrapping_add(12345);
            0.55 * (k as f32 / 12500.0 * std::f32::consts::TAU).sin() + ((r >> 16) as f32 / 65536.0 - 0.5) * 0.15
        })
        .collect();
    for pre in [6250 + 130, 6250 - 40, 6250, 18750 + 200] {
        let (e, any) = edge_smooth(
            v.as_slice(),
            pre,
            crate::fastdraw::EdgeSearch { level: 0.0, hysteresis: 0.06, rising: true, either: false },
            31_250,
            62,
            |_| true,
        );
        let t = e.unwrap();
        assert!((t - 12500.0).abs() < 60.0 || (t - 25000.0).abs() < 60.0, "pre {}: {}", pre, t);
        assert!(!any.unwrap().rising, "pre {}: the nearest edge is the falling one", pre);
    }
    // at the rising edge itself
    assert!(
        (edge_smooth(
            v.as_slice(),
            12_480,
            crate::fastdraw::EdgeSearch { level: 0.0, hysteresis: 0.06, rising: true, either: false },
            31_250,
            62,
            |_| true
        )
        .0
        .unwrap()
            - 12500.0)
            .abs()
            < 60.0
    );
    // falling, nearer the one at 6250 than the one at 18750 (from 12480 both are about as far: the comparators fire
    // at the first crossing, so the noise decides which is nearer)
    assert!(
        (edge_smooth(
            v.as_slice(),
            11_000,
            crate::fastdraw::EdgeSearch { level: 0.0, hysteresis: 0.06, rising: false, either: false },
            31_250,
            62,
            |_| true
        )
        .0
        .unwrap()
            - 6250.0)
            .abs()
            < 60.0
    );
}

// Break it catches: the software trigger arming on noise the hardware ignores. A 0..5 V square with +-0.1 V noise
// and the level 0.15 V above the low rail at 1 V/div (h = 0.3 V): the hardware never arms a rising edge (it needs
// a sample below -0.15 V), so it forces frames, and the software must not find "edges" in that noise
#[test]
fn a_level_on_the_noise_of_a_rail_finds_no_edge() {
    let mut r = 777u32;
    let v: Vec<f32> = (0..20_000)
        .map(|k| {
            r = r.wrapping_mul(1_103_515_245).wrapping_add(12345);
            (if (k / 5000) % 2 == 1 { 5.0 } else { 0.0 }) + ((r >> 16) as f32 / 65536.0 - 0.5) * 0.2
        })
        .collect();
    for l in [1usize, 4, 16] {
        for pre in [1000, 2500, 12_000] {
            let (e, any) = edge_smooth(
                v.as_slice(),
                pre,
                crate::fastdraw::EdgeSearch { level: 0.15, hysteresis: 0.3, rising: true, either: false },
                9000,
                l,
                |_| true,
            );
            assert_eq!(e, None, "l {} pre {}", l, pre);
            // the falling edges arm (from 5 V) and fire
            assert!(any.is_some_and(|x| !x.rising), "l {} pre {}", l, pre);
        }
    }
    // the level in the middle: the rising edges are found at their crossings
    let (e, _) = edge_smooth(
        v.as_slice(),
        5100,
        crate::fastdraw::EdgeSearch { level: 2.5, hysteresis: 0.3, rising: true, either: false },
        9000,
        1,
        |_| true,
    );
    assert!((e.unwrap() - 5000.0).abs() < 1.0);
}

/// A falling edge with noise crosses the level upwards too: with hysteresis that is no rising edge, and the rising
/// edge half a period away is found.
#[test]
fn hysteresis_keeps_noise_on_the_other_edge_out() {
    // a sine, 1000 samples a period, with a +-0.02 wiggle; it falls through 0 at 500
    let v: Vec<f32> = (0..3000)
        .map(|k| (k as f32 / 1000.0 * std::f32::consts::TAU).sin() + if k % 2 == 0 { 0.02 } else { -0.02 })
        .collect();
    // without hysteresis the wiggle at the falling crossing looks like a rising edge
    assert!((edge_near(&v, 500, 0.0, 0.0, true, false, 16).unwrap() - 500.0).abs() < 3.0);
    // with it, nothing rising near 500 ...
    assert!(edge_near(&v, 500, 0.0, 0.1, true, false, 16).is_none());
    // ... and the rising crossing at 1000 within half a period
    assert!((edge_near(&v, 500, 0.0, 0.1, true, false, 600).unwrap() - 1000.0).abs() < 3.0);
    // the falling one is where the hardware said
    assert!((edge_near(&v, 500, 0.0, 0.1, false, false, 16).unwrap() - 500.0).abs() < 6.0);
}
