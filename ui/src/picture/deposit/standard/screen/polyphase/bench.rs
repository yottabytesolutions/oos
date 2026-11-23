// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::model::Model;
use crate::picture::deposit::kernel::Vertical;
use std::{collections::BTreeSet, hint::black_box, time::Instant};

const WIDTH: usize = 970;
const HEIGHT: usize = 448;

const CASES: [(&str, usize, usize, f64, f64); 3] = [
    // Sample rates are 1.25 GS/s, 312.5 MS/s and 1.25 GS/s respectively.
    ("short-1ch-5ns", 72, 1, 62.5, 4.381),
    ("short-4ch-50ns", 176, 4, 156.25, 9.381),
    ("hires-200ns", 2816, 1, 2500.0, 158.381),
];

fn measure(mut run: impl FnMut()) -> f64 {
    const REPEATS: u32 = 500;
    run();
    let mut elapsed = std::array::from_fn::<_, 9, _>(|_| {
        let start = Instant::now();
        for _ in 0..REPEATS {
            run();
        }
        start.elapsed().as_secs_f64() * 1e9 / f64::from(REPEATS)
    });
    elapsed.sort_by(f64::total_cmp);
    elapsed[elapsed.len() / 2]
}

fn signal(i: usize) -> u16 {
    let phase = std::f64::consts::TAU * 0.297 * i as f64;
    (32768.0 + 15500.0 * phase.sin() + 9000.0 * (phase / 3.0 + 1.2).sin()) as u16
}

fn codes(words: &[u16], lanes: usize, lane: usize) -> Codes<'_> {
    Codes {
        bytes: &[],
        words: Some(words),
        stride: lanes,
        word: lane,
        n: words.len() / lanes,
        affine: (1.0 / 16384.0, 0.0),
        corrected: None,
        volt: |code, _| (f32::from(code) - 32768.0) / 16384.0,
    }
}

fn screen(name: &str, len: usize, lanes: usize, span: f64, start: f64) {
    let stride = span / POINTS as f64;
    let geom = (position(start), position(stride));
    let lanczos = stride < 0.5;
    let phases: BTreeSet<_> = (0..POINTS)
        .map(|i| rounded_phase(geom.0 + geom.1 * i as i64, if lanczos { PHASE_BITS } else { COEFFICIENT_BITS }))
        .collect();
    let (rows, runs) = if lanczos {
        let mut runs = 0;
        let mut last = None;
        let rows: BTreeSet<_> = (0..POINTS)
            .map(|k| {
                let x = geom.0 + geom.1 * k as i64;
                let i = (x >> POSITION_BITS) as usize;
                if last.replace(i) != Some(i) {
                    runs += 1;
                }
                let edge = if i + 8 >= len { i + 16 - len } else { 7usize.saturating_sub(i) };
                (edge, rounded_phase(x, PHASE_BITS))
            })
            .collect();
        (rows.len(), runs)
    } else {
        (0, 0)
    };
    let words: Vec<_> = (0..len * lanes).map(signal).collect();
    let map = Map { ti: 0.0, x_per_sample: 1.0, x0: 0.0, y_per_volt: -112.0, y0: 224.0 };
    let mut scratch = Scratch::default();
    let projection = crate::picture::deposit::standard::projection(&Model::default(), WIDTH, HEIGHT, 1.0);
    let window = projection.time_per_div * f64::from(crate::model::HDIV);
    scratch.edges(&projection, -window / 2.0, window / POINTS as f64);
    let interpolate_ns = measure(|| {
        for c in 0..lanes {
            let sample = codes(&words, lanes, c);
            let row = RowScale::new(&sample, &map);
            interpolate(black_box(&mut scratch), &sample, black_box(geom), lanczos, &row);
            black_box(&scratch.vertices);
        }
    });
    let mut out = Spans::default();
    let mix_ns = measure(|| {
        for _ in 0..lanes {
            spans_into(&scratch.edges, 0, black_box(&scratch.vertices), HEIGHT, &mut out);
            black_box(&out);
        }
    });
    println!(
        "{name}: {lanes}x{len}, {POINTS} vertices/lane, step={stride:.9}, phases={}, interpolate={interpolate_ns:.0} ns, mix={mix_ns:.0} ns",
        phases.len()
    );
    println!("  coefficient bytes={}, sample runs/lane={runs}", rows * size_of::<Taps>());
}

fn zoom() {
    let len = 31_250;
    let stride = 1.5625 / POINTS as f64;
    let start = 15_625.381;
    let phase_bits = 32 - PHASE_BITS;
    let start_fixed = (start * (1u64 << 32) as f64).round() as i64;
    let step_fixed = (stride * (1u64 << 32) as f64).round() as i64;
    let phases: BTreeSet<_> = (0..POINTS)
        .map(|i| (((start_fixed + step_fixed * i as i64) & ((1i64 << 32) - 1)) + (1 << (phase_bits - 1))) >> phase_bits)
        .collect();
    let words: Vec<_> = (0..len * NCH).map(signal).collect();
    let mut filter = Filter::default();
    let mut out = Spans::default();
    let window = Window { start, step: stride, column_origin: 0.0 };
    let vertical = Vertical { volts_per_div: 0.275, offset: 0.0 };
    let ns = measure(|| {
        for c in 0..NCH {
            filter.spans(&codes(&words, NCH, c), black_box(window), (WIDTH, HEIGHT / 2), vertical, &mut out);
            black_box(&out);
        }
    });
    println!(
        "zoom detail: {NCH}x{len}, step={stride:.9}, {POINTS} potential vertices/lane, phases={}, spans={ns:.0} ns",
        phases.len()
    );
}

#[test]
#[ignore = "release microbenchmark for representative synthetic screen and zoom shapes"]
fn screen_resample_microbenchmark() {
    for (name, len, lanes, span, start) in CASES {
        screen(name, len, lanes, span, start);
    }
    zoom();
}
