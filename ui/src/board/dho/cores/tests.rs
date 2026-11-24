// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

/// The pattern of CH1 alone on the unit (2026-11-14), codes.
const UNIT: [f64; P] = [76.0, 32.0, 17.0, -44.0, -54.0, -5.0, -32.0, 12.0];

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
    fn gauss(&mut self) -> f64 {
        (0..12).map(|_| self.next()).sum::<f64>() - 6.0
    }
}

/// A frame of n words: a sine of `amp` codes at `cyc` cycles a sample, noise of 90 codes, the pattern starting at
/// phase `turn`.
fn frame(rng: &mut Rng, n: usize, amp: f64, cyc: f64, turn: usize, pat: &[f64; P]) -> Vec<u16> {
    let ph = rng.next() * std::f64::consts::TAU;
    (0..n)
        .map(|k| {
            (32768.0
                + amp * (std::f64::consts::TAU * cyc * k as f64 + ph).sin()
                + 90.0 * rng.gauss()
                + pat[(k + turn) % P])
                .round()
                .clamp(0.0, 65535.0) as u16
        })
        .collect()
}

/// The pattern left in a corrected frame, measured on a long copy without signal: rms of the 8 phases.
fn left(pat: &[f64; P]) -> f64 {
    (pat.iter().map(|x| x * x).sum::<f64>() / P as f64).sqrt()
}

/// Frames with the pattern at random turns, signals of several frequencies (also near a core frequency): after a
/// few frames each corrected frame keeps less than a tenth of the pattern, and the turn is found.
#[test]
fn learns_the_pattern_and_takes_it_off() {
    let mut rng = Rng(7);
    let mut cores = Cores::default();
    let key = (1, 0, 1_250_000_000);
    let mut worst = 0.0f64;
    for i in 0..60 {
        let turn = (rng.next() * 8.0) as usize % P;
        let n = [12500, 1000, 3000, 62500][i % 4];
        let cyc = [0.0008, 0.013, 0.31, 0.1249, 0.37][i % 5];
        let mut w = frame(&mut rng, n, 15000.0, cyc, turn, &UNIT);
        let t = cores.apply(key, &mut w);
        if i >= 10 {
            // the remaining pattern: correction error = applied turn of L against the true one
            let (l, _) = cores.pattern(key).unwrap();
            // a sine near a core frequency (0.1249) may be left as it is, every other frame is corrected
            let Some(t) = t else {
                assert!(cyc == 0.1249, "frame {i} ({n} words, {cyc} cycles a sample) left as it is");
                continue;
            };
            // word k got l[(k + t) % 8] taken off and had UNIT[(k + turn) % 8]
            let err: [f64; P] = std::array::from_fn(|k| UNIT[(k + turn) % P] - l[(k + t) % P]);
            worst = worst.max(left(&err));
        }
    }
    assert!(worst < 0.1 * left(&UNIT), "left {worst:.1} of {:.1} codes rms", left(&UNIT));
}

/// A first frame with a strong sine on a core frequency teaches a wrong pattern: the frames after it do not fit,
/// are left as they are, and after `MISFITS` of them the pattern is learnt again and corrects.
#[test]
fn recovers_from_a_bad_first_frame() {
    let mut rng = Rng(5);
    let mut cores = Cores::default();
    let key = (1, 0, 1_250_000_000);
    let mut w = frame(&mut rng, 12500, 15000.0, 0.125, 0, &UNIT);
    cores.apply(key, &mut w);
    let mut fixed = None;
    for i in 0..40 {
        let turn = (rng.next() * 8.0) as usize % P;
        let mut w = frame(&mut rng, 12500, 15000.0, 0.013, turn, &UNIT);
        let before = left(&estimate(&w).unwrap().0);
        let t = cores.apply(key, &mut w);
        let after = left(&estimate(&w).unwrap().0);
        // never worse than without the correction
        assert!(after < before * 1.05 + 2.0, "frame {i}: {before:.1} -> {after:.1} codes rms");
        if t.is_some() && after < 0.1 * before && fixed.is_none() {
            fixed = Some(i);
        }
    }
    assert!(fixed.is_some_and(|i| i <= MISFITS as usize + 2), "{fixed:?}");
}

/// Without a pattern (cores that match), the correction stays at the noise: it adds next to nothing.
#[test]
fn matched_cores_stay_as_they_are() {
    let mut rng = Rng(11);
    let mut cores = Cores::default();
    let key = (1, 0, 1_250_000_000);
    for i in 0..40 {
        let mut w = frame(&mut rng, 12500, 15000.0, [0.0008, 0.31, 0.37][i % 3], 0, &[0.0; P]);
        cores.apply(key, &mut w);
    }
    let (l, _) = cores.pattern(key).unwrap();
    assert!(left(&l) < 3.0, "{l:?}");
}

/// The period-2 pattern of four channels and a frame too short to take part.
#[test]
fn period_two_and_short_frames() {
    let mut rng = Rng(3);
    let mut cores = Cores::default();
    let key = (15, 0, 312_500_000);
    let pat = [90.0, -90.0, 90.0, -90.0, 90.0, -90.0, 90.0, -90.0];
    for _ in 0..20 {
        let turn = (rng.next() * 8.0) as usize % P;
        let mut w = frame(&mut rng, 1000, 15000.0, 0.013, turn, &pat);
        let before = estimate(&w).unwrap().0;
        cores.apply(key, &mut w);
        let after = estimate(&w).unwrap().0;
        assert!(left(&after) < 0.15 * left(&before), "{before:?} -> {after:?}");
    }
    let mut short = frame(&mut rng, 100, 15000.0, 0.013, 1, &pat);
    let copy = short.clone();
    assert_eq!(cores.apply(key, &mut short), None);
    assert_eq!(short, copy);
}

fn interleave(lanes: &[Vec<u16>]) -> Vec<[u8; 2]> {
    (0..lanes[0].len()).flat_map(|i| lanes.iter().map(move |lane| lane[i].to_le_bytes())).collect()
}

#[test]
fn bounded_interleaved_estimate_corrects_each_physical_turn_with_a_signal_ramp() {
    for ratio in [1, 2, 4] {
        let period = P / ratio;
        let mut offsets = UNIT;
        fold_period(&mut offsets, period);
        let matcher = Matcher::new(Learnt { l: offsets, n: 50_000.0, misfit: 0 }, period).unwrap();
        for len in [130, 513, 4097] {
            for turn in 0..period {
                let lanes: Vec<_> = (0..ratio)
                    .map(|lane| {
                        (0..len)
                            .map(|i| {
                                ((1000 + 7 * i + lane) as u16)
                                    .saturating_add_signed(offsets[(i + turn + lane) % period].round() as i16)
                            })
                            .collect()
                    })
                    .collect();
                let mut words = interleave(&lanes);
                words.truncate(words.len() - (ratio - 1));
                for (lane, samples) in lanes.iter().enumerate() {
                    let correction = matcher.correction(&words, lane, ratio).unwrap();
                    let expected = Correction::new(&offsets, (turn + lane) % period);
                    assert_eq!(correction.offsets(), expected.offsets());
                    for (i, &sample) in samples.iter().enumerate() {
                        assert_eq!(correction.word(i, sample), (1000 + 7 * i + lane) as u16);
                    }
                }
            }
        }
    }
}
