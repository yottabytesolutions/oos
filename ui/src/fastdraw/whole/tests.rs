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

// Count the writes on the real scatter and sweep branches. A dense sweep uses the
// eight-lane cost model. A narrow span must stay on the cheaper scatter route.
#[test]
#[ignore = "host span work: set OOS_PERF_TESTS=1"]
fn perf_guard_deposit_spans() {
    assert_eq!(std::env::var("OOS_PERF_TESTS").as_deref(), Ok("1"));
    for n in [100_000, 12_512] {
        for (name, values) in NAMES.into_iter().zip(signals(n)) {
            span_write_budget(name, &values);
        }
    }
}

const NAMES: [&str; 9] = ["sine", "two sines", "square", "ramp", "clipped", "flat", "noise", "noisy square", "gaps"];

fn work_map(samples: usize) -> Map {
    Map {
        ti: samples as f64 / 2.0,
        x_per_sample: 970.0 / samples as f64 * 0.8,
        x0: 485.0,
        y_per_volt: -56.0,
        y0: 224.0,
    }
}

fn span_write_budget(name: &str, values: &[f32]) {
    let (w, h) = (970, 448);
    let (mut reference, mut actual) = (vec![0; w * h], vec![0; w * h]);
    let m = work_map(values.len());
    deposit_reference(&mut reference, w, h, values, &m, 1);
    let work = work::count_spans(|| {
        let _ = deposit_samples::<true>(&mut actual, w, h, &work::CountedSamples(values), &m, 1);
    });
    assert!(actual == reference, "{name}: span pixels changed");
    let scattered = reference.iter().filter(|&&pixel| pixel != 0).count();
    let rows = reference.chunks_exact(w).map(|row| row.iter().any(|&pixel| pixel != 0));
    let (first, last) = (
        rows.clone().position(|used| used).unwrap(),
        rows.rev().position(|used| used).map(|last| h - 1 - last).unwrap(),
    );
    let width = crate::fastdraw::columns(w, values.len(), &m).count();
    assert_eq!((work.samples, work.joins), (values.len(), width - 1), "{name}: one span scan");
    let sweep = (last - first + 1) * width;
    let expected = if sweep / 8 < scattered { (0, sweep) } else { (scattered, 0) };
    assert_eq!((work.scattered, work.contiguous), expected, "{name}: use the cheaper raster write pattern");
    let ratio = (work.scattered + work.contiguous.div_ceil(8)) as f64 / scattered as f64;
    println!("SPAN_WORK points={} signal={name:?} ratio={ratio:.6} {work:?}", values.len());
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

// Runs save writes on repeated rows. Noisy columns should only probe their first
// eight samples before continuing with the single-add kernel.
#[test]
#[ignore = "host dot work: set OOS_PERF_TESTS=1"]
fn perf_guard_dots() {
    assert_eq!(std::env::var("OOS_PERF_TESTS").as_deref(), Ok("1"));
    let (w, h, n) = (970, 448, 100_000);
    for (name, values) in NAMES.into_iter().zip(signals(n)) {
        let (mut reference, mut actual) = (vec![0; w * h], vec![0; w * h]);
        let m = work_map(n);
        deposit_dots_reference(&mut reference, w, h, &values, &m, 1);
        let work = work::count_dots(|| {
            let _ = dots::draw::<true>(&mut actual, w, h, &values, &m, 1);
        });
        assert!(actual == reference, "{name}: dots pixels changed");
        let writes: usize = reference.iter().map(|&count| usize::from(count)).sum();
        assert_eq!(work.samples, n, "{name}: visit each sample once");
        assert!(work.writes <= writes && work.probes <= n, "{name}: {work:?}");
        let write_limit = match name {
            "flat" | "square" | "ramp" => writes / 20,
            "sine" => writes * 4 / 5,
            "gaps" => writes * 3 / 5,
            _ => writes,
        };
        assert!(work.writes <= write_limit, "{name}: repeated rows must coalesce: {work:?}");
        let probe_limit = match name {
            "noise" | "two sines" => 8 * w,
            "noisy square" => 16 * w,
            "clipped" => n / 2,
            _ => n,
        };
        assert!(work.probes <= probe_limit, "{name}: bound repeat probes: {work:?}");
        println!("DOT_WORK signal={name:?} write_ratio={:.6} {work:?}", work.writes as f64 / writes as f64);
    }
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
