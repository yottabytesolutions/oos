// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::drawpipe::CodeLayout;
use crate::fastdraw::Samples;

#[test]
fn band_limited_detail_matches_the_exact_screen_within_one_row() {
    const WIDTH: usize = 970;
    const HEIGHT: usize = 224;
    const LEN: usize = 2000;
    let mut filter = Filter::default();
    let mut actual = Spans::default();
    for seed in 0..12 {
        let signal = |i| {
            let wave = |frequency: f64, phase: f64| (std::f64::consts::TAU * (frequency * i as f64 + phase)).sin();
            let phase = seed as f64 * 0.173;
            (32_768.0
                + 6_000.0 * wave(0.4, phase)
                + 2_000.0 * wave(0.213, phase * 1.7)
                + 500.0 * wave(0.047, phase * 2.1))
            .round() as u16
        };
        let bytes: Vec<u8> = (0..LEN).flat_map(|i| signal(i).to_le_bytes()).collect();
        let layout = CodeLayout::uncorrected(1, [Some(0), None, None, None], [(1.0, 0.0); 4], |code, _| {
            (f32::from(code) - 32_768.0) / 7500.0
        });
        let codes = layout.channel(&bytes, 0);
        let values: Vec<_> = (0..codes.n).map(|i| codes.at(i)).collect();
        for samples_per_column in [0.016, 0.031, 0.23, 0.85, 1.99] {
            let duration = WIDTH as f64 * samples_per_column;
            for start in [-3.107, 0.132, 14.321, LEN as f64 - duration + 2.959] {
                let step = duration / crate::sparse::POINTS as f64;
                let volts_per_div = if seed % 2 == 0 { 0.275 } else { 0.55 };
                let vertical = Vertical { volts_per_div, offset: (seed as f32 - 5.0) * 0.031 };
                let window = Window { start, step, column_origin: 0.0 };
                filter.spans(&codes, window, (WIDTH, HEIGHT), vertical, &mut actual);
                if seed % 2 != 0 && samples_per_column <= 0.031 && start == 14.321 {
                    assert_eq!(filter.points.len(), WIDTH + 1, "exercise the column-boundary path");
                }
                let mut reference = vec![(i16::MAX, i16::MIN); WIDTH];
                exact_spans(&values, (start, step, duration), (WIDTH, HEIGHT), vertical, &mut reference);
                for (x, (&(a, b), &(lo, hi))) in actual.rows.iter().zip(&reference).enumerate() {
                    let error = match (a <= b, lo <= hi) {
                        (true, true) => a.abs_diff(lo).max(b.abs_diff(hi)),
                        (true, false) => (b as usize + 1).min(HEIGHT - a as usize) as u16,
                        (false, true) => (hi as usize + 1).min(HEIGHT - lo as usize) as u16,
                        (false, false) => 0,
                    };
                    assert!(error <= 1, "seed {seed}, start {start}, step {step}, column {x}: {a}..{b} vs {lo}..{hi}");
                }
                assert_eq!(actual.rows.len(), WIDTH);
            }
        }
    }
}

#[test]
fn samples_outside_the_window_and_filter_support_cannot_change_the_detail() {
    let layout = CodeLayout::uncorrected(1, [Some(0), None, None, None], [(1.0, 0.0); 4], |code, _| {
        (f32::from(code) - 32_768.0) / 7500.0
    });
    let mut filter = Filter::default();
    let mut spans = Spans::default();
    let vertical = Vertical { volts_per_div: 0.3, offset: 0.0 };
    let window = Window { start: 1000.25, step: 0.004, column_origin: 0.0 };
    let data = |outside| {
        (0..35_160)
            .flat_map(|i| {
                let word: u16 = if (993..=1022).contains(&i) { 30_000 + (i % 31) as u16 * 200 } else { outside };
                word.to_le_bytes()
            })
            .collect::<Vec<_>>()
    };
    filter.spans(&layout.channel(&data(0), 0), window, (970, 224), vertical, &mut spans);
    let reference = spans.rows.clone();
    filter.spans(&layout.channel(&data(u16::MAX), 0), window, (970, 224), vertical, &mut spans);
    assert_eq!(spans.rows, reference);
}

fn exact_spans(
    values: &[f32],
    (start, step, duration): (f64, f64, f64),
    (width, height): (usize, usize),
    vertical: Vertical,
    out: &mut [(i16, i16)],
) {
    let mut points = Vec::new();
    crate::board::record::resample_channel(
        values,
        (1.0, false),
        crate::model::AcqMode::Normal,
        (crate::sparse::POINTS, step, -duration / 2.0),
        start + duration / 2.0,
        &mut points,
    );
    crate::wave::trace_columns(
        width,
        height,
        (vertical.volts_per_div, vertical.offset),
        false,
        (duration / f64::from(crate::model::HDIV), 0.0),
        -duration / 2.0,
        step,
        &points,
        24,
        None,
        false,
        (0, width as i32),
        |x, a, b, _| {
            let span = &mut out[x as usize];
            *span = (span.0.min(a as i16), span.1.max(b as i16));
        },
    );
}

#[test]
fn sample_runs_keep_every_zoom_knot_including_clipped_taps_and_the_last_knot() {
    let layout = CodeLayout::uncorrected(1, [Some(0), None, None, None], [(1.0, 0.0); 4], |code, _| f32::from(code));
    let bytes: Vec<u8> = (0..83).flat_map(|i| ((13237 * i + 15289) as u16).to_le_bytes()).collect();
    let mut filter = Filter::default();
    for len in [3, 16, 83] {
        let codes = layout.channel(&bytes[..len * 2], 0);
        let unit = SAMPLE_ONE as f64;
        for (start, step, last) in
            [(0.0, 0.29, 1.0), (0.17, 0.017, 2.0), (1.0, 0.043, 2.0), (0.1, 0.29, (len - 1) as f64)]
        {
            let (start, step, last) = ((start * unit) as i64, (step * unit) as i64, (last * unit) as i64);
            let count = ((last - start) / step) as usize + 4;
            filter.interpolate(&codes, (start, step, last), count);
            for (i, &actual) in filter.points.iter().enumerate() {
                assert_eq!(actual, reference_knot(&codes, (start + step * i as i64).min(last)), "len={len} knot={i}");
            }
            assert_eq!(filter.points.len(), count);
        }
    }
}

fn reference_knot(codes: &Codes<'_>, x: i64) -> i32 {
    let i = (x >> SAMPLE_BITS) as usize;
    let shift = SAMPLE_BITS - PHASE_BITS;
    let phase = (((x & (SAMPLE_ONE - 1)) + (1 << (shift - 1))) >> shift) as usize;
    let weights = taps(phase);
    let samples = std::array::from_fn(|tap| {
        (i + tap)
            .checked_sub(LEFT)
            .filter(|&index| index < codes.n)
            .map_or(0, |index| (codes.at_code(index) ^ 0x8000) as i16)
    });
    let sum = crate::picture::deposit::interpolation::scalar_dot(&samples, &weights);
    let valid = LEFT.saturating_sub(i)..(codes.n + LEFT - i).min(TAPS);
    if valid.len() == TAPS {
        sum
    } else {
        normalise(sum, &weights[valid])
    }
}
