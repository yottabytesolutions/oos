// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::fastdraw::{Codes, CorrectedCodes};
use crate::picture::deposit::kernel::SamplesRef;
use std::hint::black_box;
use std::time::Instant;

const CAL: CorrectedCodes = CorrectedCodes { offsets: [3, -2, 4, 0, -1, 2, -3, 1], scale: 1.25, offset: 0.0 };

fn words(v: &[f32], stride: usize) -> Vec<u16> {
    let mut words = vec![32768; stride * v.len()];
    for (i, &x) in v.iter().enumerate() {
        let code = (f64::from(x) / CAL.scale * 7500.0 + 32768.0).round() as i32;
        words[i * stride] = (code + CAL.offsets[i % 8]) as u16;
    }
    words
}

fn codes(words: &[u16], stride: usize) -> SamplesRef<'_> {
    SamplesRef::Codes(Codes {
        bytes: &[],
        words: Some(words),
        stride,
        word: 0,
        n: words.len() / stride,
        affine: (0.0, 0.0),
        corrected: Some(CAL),
        volt: |_, _| unreachable!("the benchmark uses corrected codes"),
    })
}

fn micros<T>(count: usize, mut work: impl FnMut(usize) -> T) -> f64 {
    for i in 0..count {
        black_box(work(i));
    }
    let start = Instant::now();
    for i in 0..(count * 128) {
        black_box(work(i % count));
    }
    start.elapsed().as_secs_f64() * 1e6 / (count * 128) as f64
}

fn bench_shape(stride: usize, n: usize, signal: &str, v: &[f32]) {
    let trigger = EdgeSearch { level: 0.0, hysteresis: 0.06, rising: true, either: false };
    let anchors = oos_dsp::edges::trigger_edges(v, trigger.level, trigger.hysteresis);
    let records: Vec<(Vec<u16>, EdgeSearch, usize)> = anchors
        .iter()
        .filter(|e| (n / 2..v.len() - n / 2).contains(&(e.t as usize)))
        .take(24)
        .enumerate()
        .map(|(i, e)| {
            let a = e.t as usize - n / 2;
            (words(&v[a..a + n], stride), EdgeSearch { rising: e.rising, ..trigger }, n / 2 + i % 7 - 3)
        })
        .collect();
    assert!(!records.is_empty(), "no hardware points for {signal} at {n} samples");
    for spc in [3.1, 6.25, 30.0f64] {
        let span = ((450.0 * spc) as usize).min(n / 2);
        let l = spc as usize;
        let near = NEAR_COLS * l + NEAR_PAD;
        let try_near = |i: usize| {
            let (words, t, pre) = &records[i];
            (near < span).then(|| near_only(&codes(words, stride), (0, n), (*pre, near, l), *t, &|_| true)).flatten()
        };
        let fallback = (0..records.len()).filter(|&i| try_near(i).is_none()).count();
        let total_us = micros(records.len(), |i| {
            let (words, t, pre) = &records[i];
            edge_smooth_near(
                &black_box(codes(words, stride)),
                *pre,
                t.level,
                t.hysteresis,
                t.rising,
                t.either,
                span,
                l,
                |_| true,
            )
        });
        let wide_us = micros(records.len(), |i| {
            let (words, t, pre) = &records[i];
            smooth_all(&black_box(codes(words, stride)), *pre, *t, span, l, &|_| true)
        });
        let near_us = micros(records.len(), |i| try_near(black_box(i)));
        let (convert, smooth, scan) = bench_stages(&records, stride, (n, span, l));
        println!(
            "{stride}x{n},{signal},{spc},{}/{},{near_us:.3},{wide_us:.3},{total_us:.3},{convert:.3},{smooth:.3},{scan:.3}",
            fallback, records.len()
        );
    }
}

fn bench_stages(
    records: &[(Vec<u16>, EdgeSearch, usize)],
    stride: usize,
    (n, span, l): (usize, usize, usize),
) -> (f64, f64, f64) {
    let mut converted = vec![0.0; n];
    let convert = micros(records.len(), |i| {
        let (words, _, pre) = &records[i];
        let v = black_box(codes(words, stride));
        let a = pre.saturating_sub(span + l);
        let b = (pre + span + l).min(n);
        for (j, x) in converted[a..b].iter_mut().enumerate() {
            *x = v.at(a + j);
        }
        black_box(&converted[a..b]);
    });
    let decoded: Vec<Vec<f32>> = records
        .iter()
        .map(|(words, _, _)| {
            let v = codes(words, stride);
            (0..n).map(|j| v.at(j)).collect()
        })
        .collect();
    let (mut samples, mut window, mut edges) = (Vec::new(), Vec::new(), Vec::new());
    let smooth = micros(records.len(), |i| {
        let (_, t, pre) = &records[i];
        let bounds = sample_bounds(n, (*pre, span, l)).unwrap();
        smoothed(black_box(decoded[i].as_slice()), 0, bounds, l, (t.level, 0.0), (&mut samples, &mut window));
        black_box(&samples);
    });
    let averaged: Vec<Vec<f32>> = decoded
        .iter()
        .zip(records)
        .map(|(v, (_, t, pre))| {
            let bounds = sample_bounds(n, (*pre, span, l)).unwrap();
            smoothed(v.as_slice(), 0, bounds, l, (t.level, 0.0), (&mut samples, &mut window));
            samples.clone()
        })
        .collect();
    let scan = micros(records.len(), |i| {
        let (_, t, _) = records[i];
        oos_dsp::edges::trigger_edges_into(black_box(&averaged[i]), t.level, t.hysteresis, &mut edges);
        black_box(&edges);
    });
    (convert, smooth, scan)
}

#[test]
#[ignore = "host timing: set OOS_PERF_TESTS=1"]
fn trigger_refinement_by_shape() {
    assert_eq!(std::env::var("OOS_PERF_TESTS").as_deref(), Ok("1"), "set OOS_PERF_TESTS=1");
    println!("shape,signal,samples/column,fallbacks/records,near us,wide us,near-first us,wide conversion us,wide smoothing us,wide comparator us");
    let corpus = tests::corpus();
    for (stride, n) in [(1, 14_064), (2, 7_032), (4, 3_520)] {
        for (name, v) in corpus.iter().filter(|(name, _)| {
            matches!(name.as_str(), "square noise 0" | "square noise 0.4" | "sine noise 0" | "ringing logic noise 0.4")
        }) {
            bench_shape(stride, n, name, v);
        }
    }
}
