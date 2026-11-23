// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::fastdraw::{min_max, samples::min_max_scalar};

/// Signals for the deposit equality: sine, noisy square, noise, a ramp, clipped beyond the map, NaN gaps, flat.
fn signals(n: usize) -> Vec<Vec<f32>> {
    let mut x = 12345u32;
    let mut rnd = move || {
        x = x.wrapping_mul(1_103_515_245).wrapping_add(12345);
        ((x >> 8) & 0xffff) as f32 / 65536.0 - 0.5
    };
    let mut out = vec![
        (0..n).map(|i| 0.9 * (i as f32 / 400.0 * std::f32::consts::TAU).sin()).collect::<Vec<f32>>(),
        (0..n).map(|i| 0.7 * (i as f32 / 37.0).sin() + 0.3 * (i as f32 / 5.3).sin()).collect(),
        (0..n).map(|i| if (i / 900) % 2 == 0 { 1.5 } else { -1.5 } + 0.0).collect(),
        (0..n).map(|i| i as f32 / n as f32 * 10.0 - 5.0).collect(),
        (0..n).map(|i| 6.0 * (i as f32 / 300.0).sin()).collect(),
        vec![0.25; n],
    ];
    out.push((0..n).map(|_| rnd()).collect());
    out.push((0..n).map(|i| if (i / 700) % 2 == 0 { 1.0 } else { -1.0 } + 0.05 * rnd()).collect());
    let mut gaps: Vec<f32> = (0..n).map(|i| (i as f32 / 90.0).sin()).collect();
    for i in (0..n).step_by(97) {
        gaps[i..(i + 13).min(n)].fill(f32::NAN);
    }
    out.push(gaps);
    out
}

// Break it catches: a span pass that draws other pixels than one scattered write per pixel did (rows, columns,
// joins, clipping, NaN gaps, saturation), whichever of the two ways it adds them.
#[test]
fn deposit_is_bit_identical_to_the_scattered_writes() {
    let (w, h) = (970usize, 448usize);
    let mut cases = 0;
    let mut sweeps = 0;
    for n in [1_408usize, 12_512, 100_000] {
        for v in signals(n) {
            for (xps, ti, x0) in [
                (1.0 / 100.0, n as f64 / 2.0, 485.0),
                (0.00776, n as f64 / 2.0 + 0.37, 485.0),
                (0.7, 3.3, 10.0),
                (970.0 / n as f64, 0.0, 0.0),
                (0.0123, n as f64 * 0.9, 120.5),
                (0.25, -40.0, 0.0),
            ] {
                for (ypv, y0) in [(-56.0f32, 224.0f32), (-560.0, 224.0), (37.0, 10.0), (-5.0, 447.6)] {
                    for (wgt, pre) in [(24u16, 0u16), (1, 0), (60_000, 30_000)] {
                        let m = Map { ti, x_per_sample: xps, x0, y_per_volt: ypv, y0 };
                        let (mut a, mut b) = (vec![pre; w * h], vec![pre; w * h]);
                        deposit_reference(&mut a, w, h, &v, &m, wgt);
                        let _ = deposit(&mut b, w, h, &v, &m, wgt);
                        assert!(a == b, "n {} xps {} ti {} y {} {} wgt {}", n, xps, ti, ypv, y0, wgt);
                        let mut sp = Spans::default();
                        crate::fastdraw::spans_into(w, h, &v[..], &m, &mut sp);
                        if let Some((c0, c1, r0, r1, px)) = span_extent(&sp) {
                            sweeps += (px > sweep_cost(r1 - r0 + 1, c1 - c0)) as usize;
                        }
                        cases += 1;
                    }
                }
            }
        }
    }
    // both ways of adding were compared
    assert!(sweeps > 20 && sweeps < cases - 20, "{} of {} cases swept", sweeps, cases);
}

// Timing guard (tools/perf-guard.sh): the span pass against one scattered write per pixel, per signal, at 100 k and
// 12.5 k points over 970 columns. Never slower on short spans (square), faster on long ones (sine, noise).
#[test]
#[ignore = "host timing: set OOS_PERF_TESTS=1"]
fn perf_guard_deposit_spans() {
    assert_eq!(std::env::var("OOS_PERF_TESTS").as_deref(), Ok("1"), "set OOS_PERF_TESTS=1");
    let (w, h) = (970usize, 448usize);
    let names = ["sine", "two sines", "square", "ramp", "clipped", "flat", "noise", "noisy square", "gaps"];
    let mut worst: f64 = 0.0;
    for n in [100_000usize, 12_512] {
        for (name, v) in names.iter().zip(signals(n)) {
            let m = Map {
                ti: n as f64 / 2.0,
                x_per_sample: 970.0 / n as f64 * 0.8,
                x0: 485.0,
                y_per_volt: -56.0,
                y0: 224.0,
            };
            let (mut ma, mut mb) = (vec![0u16; w * h], vec![0u16; w * h]);
            let (old, new) = crate::perfguard::guard_pair(
                1.25,
                crate::perfguard::ROUNDS,
                33,
                || deposit_reference(&mut ma, w, h, &v, &m, 1),
                || {
                    std::hint::black_box(deposit(&mut mb, w, h, &v, &m, 1));
                },
            );
            let r = new.as_secs_f64() / old.as_secs_f64();
            worst = worst.max(r);
            println!("deposit {} pt {}: one write per pixel {:?}, span pass {:?} (ratio {:.2})", n, name, old, new, r);
        }
    }
    assert!(worst <= 1.25, "the span pass must not be slower: worst ratio {:.2}", worst);
}

// Break it catches: dots counted in runs drawing other pixels or other weights than one add per sample (runs
// across columns, off-map samples, NaN gaps, saturation, a run cut by an off-map sample)
#[test]
fn dots_are_bit_identical_to_one_add_per_sample() {
    let (w, h) = (970usize, 448usize);
    for n in [1_408usize, 12_512, 100_000] {
        for v in signals(n) {
            for (xps, ti, x0) in [
                (1.0 / 100.0, n as f64 / 2.0, 485.0),
                (0.00776, n as f64 / 2.0 + 0.37, 485.0),
                (0.7, 3.3, 10.0),
                (0.0123, n as f64 * 0.9, 120.5),
            ] {
                for (ypv, y0) in [(-56.0f32, 224.0f32), (-560.0, 224.0), (37.0, 10.0), (-5.0, 447.6)] {
                    for (wgt, pre) in [(24u16, 0u16), (60_000, 30_000), (1, 65_534)] {
                        let m = Map { ti, x_per_sample: xps, x0, y_per_volt: ypv, y0 };
                        let (mut a, mut b) = (vec![pre; w * h], vec![pre; w * h]);
                        deposit_dots_reference(&mut a, w, h, &v, &m, wgt);
                        let rows = deposit_dots(&mut b, w, h, &v, &m, wgt);
                        if pre == 0 {
                            let mut touched = a
                                .chunks(w)
                                .enumerate()
                                .filter_map(|(r, row)| row.iter().any(|&value| value != 0).then_some(r));
                            let expected =
                                touched.next().map_or(0..0, |first| first..touched.next_back().unwrap_or(first) + 1);
                            assert_eq!(rows, expected, "n {n} xps {xps} ti {ti} y {ypv} {y0}");
                        }
                        assert!(a == b, "n {} xps {} ti {} y {} {} wgt {}", n, xps, ti, ypv, y0, wgt);
                    }
                }
            }
        }
    }
}

// Timing guard (tools/perf-guard.sh): dots in runs against one add per sample, per signal at 100 k points.
#[test]
#[ignore = "host timing: set OOS_PERF_TESTS=1"]
fn perf_guard_dots() {
    assert_eq!(std::env::var("OOS_PERF_TESTS").as_deref(), Ok("1"), "set OOS_PERF_TESTS=1");
    let (w, h) = (970usize, 448usize);
    let names = ["sine", "two sines", "square", "ramp", "clipped", "flat", "noise", "noisy square", "gaps"];
    let n = 100_000usize;
    let mut worst: f64 = 0.0;
    for (name, v) in names.iter().zip(signals(n)) {
        let m =
            Map { ti: n as f64 / 2.0, x_per_sample: 970.0 / n as f64 * 0.8, x0: 485.0, y_per_volt: -56.0, y0: 224.0 };
        let (mut ma, mut mb) = (vec![0u16; w * h], vec![0u16; w * h]);
        let (old, new) = crate::perfguard::guard_pair(
            1.25,
            crate::perfguard::ROUNDS,
            22,
            || deposit_dots_reference(&mut ma, w, h, &v, &m, 1),
            || {
                std::hint::black_box(deposit_dots(&mut mb, w, h, &v, &m, 1));
            },
        );
        let r = new.as_secs_f64() / old.as_secs_f64();
        worst = worst.max(r);
        println!("dots {} pt {}: one add per sample {:?}, runs {:?} (ratio {:.2})", n, name, old, new, r);
    }
    assert!(worst <= 1.25, "dots in runs must not be slower: worst ratio {:.2}", worst);
}

#[test]
fn min_max_equals_the_chain() {
    for v in signals(1000) {
        for (a, b) in [(0usize, 0usize), (0, 1), (3, 19), (0, 1000), (17, 500), (999, 1000)] {
            let (x, y) = (min_max(&v[a..b]), min_max_scalar(&v[a..b]));
            assert!(x.0 == y.0 && x.1 == y.1 || (x.0.is_nan() && y.0.is_nan()), "{:?} {:?}", x, y);
        }
    }
    assert_eq!(min_max(&[]), (f32::INFINITY, f32::NEG_INFINITY));
    assert_eq!(min_max(&[f32::NAN; 40]), (f32::INFINITY, f32::NEG_INFINITY));
}

#[test]
fn spans_per_column_and_joined_edges() {
    let (w, h) = (10, 20);
    let mut map = vec![0u16; w * h];
    // a square: 4 samples per column, low (row 15) for 5 columns, then high (row 5)
    let v: Vec<f32> = (0..40).map(|k| if k < 20 { 0.0 } else { 1.0 }).collect();
    let m = Map { ti: 0.0, x_per_sample: 0.25, x0: 0.0, y_per_volt: -10.0, y0: 15.0 };
    let _ = deposit(&mut map, w, h, &v, &m, 1);
    let col = |c: usize| (0..h).filter(|&r| map[r * w + c] > 0).collect::<Vec<_>>();
    assert_eq!(col(2), [15]);
    // column 5 holds the step: joined from row 15 (last low sample) up to row 5
    assert_eq!(col(5), (5..=15).collect::<Vec<_>>());
    assert_eq!(col(7), [5]);
    // samples off screen are skipped; a shift by ti moves the step
    let mut map2 = vec![0u16; w * h];
    let _ = deposit(&mut map2, w, h, &v, &Map { ti: 8.0, ..m }, 1);
    assert_eq!((0..h).filter(|&r| map2[r * w + 3] > 0).count(), 11);
    // dots: only the sample rows, no join across the step, 4 samples per column add up
    let mut map3 = vec![0u16; w * h];
    let _ = deposit_dots(&mut map3, w, h, &v, &m, 1);
    let col3 = |c: usize| (0..h).filter(|&r| map3[r * w + c] > 0).collect::<Vec<_>>();
    assert_eq!((col3(5), map3[5 * w + 5], col3(4)), (vec![5], 4, vec![15]));
}
