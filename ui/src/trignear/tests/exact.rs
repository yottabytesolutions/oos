// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::fastdraw::{edge_smooth, CorrectedCodes};

type EdgeBits = (Option<u64>, Option<(u64, bool)>);

fn bits((want, any): (Option<f64>, Option<Edge>)) -> EdgeBits {
    (want.map(f64::to_bits), any.map(|e| (e.t.to_bits(), e.rising)))
}

fn compare<S: TriggerSamples + ?Sized>(
    v: &S,
    (pre, span, l): (usize, usize, usize),
    trigger: EdgeSearch,
    fits: impl Fn(f64) -> bool,
) {
    let source = (0, v.len());
    let wide = || reference::search(v, source, (pre, span, l), trigger, false, &fits).unwrap_or_default();
    assert_eq!(bits(edge_smooth(v, pre, trigger, span, l, &fits)), bits(wide()));

    let EdgeSearch { level, hysteresis, rising, either } = trigger;
    let near = NEAR_COLS * l + NEAR_PAD;
    let expected = (near < span).then(|| reference::search(v, source, (pre, near, l), trigger, true, &fits)).flatten();
    let got = edge_near_window(v, 0, v.len(), pre, level, hysteresis, rising, either, span, l, &fits);
    assert_eq!(got.map(bits), expected.map(bits));
    assert_eq!(
        bits(edge_smooth_near(v, pre, level, hysteresis, rising, either, span, l, &fits)),
        bits(reference::bounded(v, (pre, span, l), trigger, &fits)),
    );
}

#[test]
fn original_edge_bits_on_the_corpus_and_nonfinite_volts() {
    let mut signals = corpus();
    let unusual = [
        f32::NEG_INFINITY,
        -0.0,
        0.0,
        f32::NAN,
        -f32::MAX,
        f32::MAX,
        f32::INFINITY,
        -f32::MIN_POSITIVE,
        f32::MIN_POSITIVE,
        -1.0,
        0.7,
        1.0,
        f32::from_bits(1),
    ]
    .repeat(173);
    signals.push(("nonfinite and extreme volts".into(), unusual));
    for (_, v) in signals {
        for pre in [0, 499, 1000, 1723, v.len() - 1] {
            for (l, span) in [(1, 700), (3, 1200), (6, 1500), (30, 1100), (256, 8800)] {
                for (level, hysteresis) in [(-0.0, 0.0), (0.3, 0.06), (-0.97, 0.3), (0.0, -0.1)] {
                    let trigger = EdgeSearch { level, hysteresis, rising: pre.is_multiple_of(3), either: pre % 3 == 1 };
                    compare(v.as_slice(), (pre, span, l), trigger, |t| t > pre.saturating_sub(116) as f64);
                }
            }
        }
    }
}

fn next(state: &mut u64) -> u16 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    (*state >> 32) as u16
}

fn compare_codes(words: &[u16], stride: usize, seed: usize) {
    let bytes: Vec<u8> = words.iter().flat_map(|word| word.to_le_bytes()).collect();
    let n = words.len() / stride;
    let even = seed.is_multiple_of(2);
    let cal = CorrectedCodes {
        offsets: [3, -45, 85, 0, -12, 200, -34, 1],
        scale: if even { 1.25 } else { -0.75 },
        // The tiny nonzero value at code 32768 prevents the exact-sum proof.
        offset: [0.0, 0.375, 1e-20][seed % 3],
    };
    let codes = Codes {
        bytes: &bytes,
        words: even.then_some(words),
        stride,
        word: seed % stride,
        n,
        affine: (0.0002, -0.125),
        corrected: (!seed.is_multiple_of(4)).then_some(cal),
        volt: oos_hal::dho::seq::volt_code,
    };
    let src = SamplesRef::Codes(codes);
    for l in [1, 3, 6, 16, 30, 47] {
        let pre = [n / 2 - 109, n / 2, n - 229][seed % 3];
        let trigger = EdgeSearch {
            level: [-0.375, 0.0, 0.125, 0.7][seed % 4],
            hysteresis: [0.0, 0.06, 0.3][seed % 3],
            rising: even,
            either: seed.is_multiple_of(3),
        };
        compare(&src, (pre, n / 2, l), trigger, |t| t > (pre - 193) as f64);
    }
}

#[test]
fn original_edge_bits_on_random_corrected_codes() {
    let mut state = 12345;
    for seed in 0..120 {
        let stride = [1, 2, 4][seed % 3];
        let n = 2048 + usize::from(next(&mut state) % 1593);
        let mut words = Vec::with_capacity(n * stride);
        for i in 0..n {
            let noise = next(&mut state);
            let block = 509 + seed % 29;
            let value = match (seed % 4, (i / block) % 4) {
                (0, _) => noise,
                (_, 0) => 25_000 + noise % 101,
                (_, 1) => 32_768 + noise % 101,
                (_, 2) => 40_000 + noise % 101,
                _ => 32_668 + noise % 101,
            };
            words.extend((0..stride).map(|_| value));
        }
        compare_codes(&words, stride, seed);
    }
}

#[test]
fn plateau_pruning_keeps_the_interpolation_predecessor() {
    let words: Vec<_> = [25_000, 32_680, 32_850, 40_000, 32_850, 32_680, 25_000]
        .into_iter()
        .flat_map(|word| std::iter::repeat_n(word, 613))
        .collect();
    let cal = CorrectedCodes { offsets: [0; 8], scale: 1.25, offset: 0.0 };
    let codes = Codes {
        bytes: &[],
        words: Some(&words),
        stride: 1,
        word: 0,
        n: words.len(),
        affine: (0.0, 0.0),
        corrected: Some(cal),
        volt: oos_hal::dho::seq::volt_code,
    };
    let trigger = EdgeSearch { level: 0.0, hysteresis: 0.1, rising: true, either: true };
    let (mut indexed, mut window) = (Vec::new(), Vec::new());
    pruned::smooth(&codes, 0, 0..words.len(), 6, trigger, (&mut indexed, &mut window));
    assert!(indexed.len() < words.len() / 8);
    for pre in [1200, 3050] {
        compare(&codes, (pre, words.len(), 6), trigger, |_| true);
        let result = edge_smooth(&codes, pre, trigger, words.len(), 6, |_| true);
        assert_eq!(result.1.unwrap().rising, pre == 1200);
    }

    let borderline = Codes { corrected: Some(CorrectedCodes { offset: 1e-20, ..cal }), ..codes };
    indexed.clear();
    pruned::smooth(&borderline, 0, 0..words.len(), 6, trigger, (&mut indexed, &mut window));
    assert!(indexed.is_empty());
    compare(&borderline, (1200, words.len(), 6), trigger, |_| true);
}

#[test]
fn narrow_trigger_matches_wide_on_noisy_code_edges_and_rails() {
    let (pre, span) = (4096, 4000);
    let mut seed = 1327;
    for rising in [false, true] {
        for offset in [-2000, -28, -1, 0, 1, 31, 2200] {
            let at = (pre as isize + offset) as usize;
            let words: Vec<_> = (0..8192)
                .map(|i| {
                    let noise = next(&mut seed) % 64;
                    if (i >= at) == rising {
                        u16::MAX - noise
                    } else {
                        noise
                    }
                })
                .collect();
            let codes = Codes {
                bytes: &[],
                words: Some(&words),
                stride: 1,
                word: 0,
                n: words.len(),
                affine: (1.0, -32768.0),
                corrected: None,
                volt: oos_hal::dho::seq::volt_code,
            };
            for l in [1, 2, 17, 108, 256] {
                for level in [0.0, 32_768.0, f32::from(u16::MAX)] {
                    let trigger = EdgeSearch { level, hysteresis: 8.0, rising, either: false };
                    let fits = &|_| true;
                    let expected = smooth_all(&codes, pre, trigger, span, l, fits);
                    let mut fallback = false;
                    let mut wide = |_, _: &dyn Fn(f64) -> bool| {
                        fallback = true;
                        expected
                    };
                    let got = edge_smooth_near_with(&codes, (pre, span, l), trigger, fits, false, Some(&mut wide));
                    assert_eq!(bits(got), bits(expected), "{rising} {offset} {l} {level}");
                    if level == 32_768.0 {
                        assert_eq!(fallback, offset.unsigned_abs() > near_span(l));
                    }
                }
            }
        }
    }
}
