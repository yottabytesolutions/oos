// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! Matching the ADC cores in software. Eight interleaved cores take turns on the samples, and each has an offset of its
//! own: on the DHO924S (2026-11-14) up to +-90 codes of the 16-bit words (7500 a division) after the stored
//! calibration, a fixed pattern that repeats every 8 samples at the full rate (a spur at 156.25 MHz, -52 dBc of a
//! 1 Vpp sine at 0.2 V/div), every 4 with two channels and every 2 with four, or when every 2nd or 4th sample is kept.
//! The pattern is the same at every V/div, but where it starts in a frame changes from frame to frame (it follows
//! the trigger point), so it is found in each frame.
//!
//! Per channel set, channel and rate, a pattern of 8 offsets is learnt from the frames: each frame's own estimate
//! (means of the 8 phases, Hann-weighted so that the signal leaks next to nothing into them) is turned to fit the
//! learnt pattern best, which then follows it a little, and the learnt pattern, turned the same way, is taken off the
//! words. A frame that does not fit any turn of it well (a short frame, a signal on the core frequencies) is left as
//! it is. The core gains differ too (about +-0.06 %), which is not corrected here.

use std::collections::HashMap;

/// Phases of the pattern: the cores at the full rate.
pub const P: usize = 8;
/// Samples of a frame the estimate looks at (the first ones).
const LOOK: usize = 1 << 16;
/// Samples a frame needs to take part.
const MIN: usize = 256;
/// Samples the learnt pattern stands for at most: a frame of n samples moves it by n / (this + n).
const MEMORY: f64 = (1 << 19) as f64;

/// Frames in a row that do not fit the learnt pattern before it is learnt again from the next one (a pattern learnt
/// from a bad first frame, a calibration meanwhile).
const MISFITS: u32 = 16;

/// What a pattern is learnt for: channel mask, channel, sample rate.
pub type Key = (u32, usize, u64);

/// A learnt pattern: offsets (codes, phase 0 first), the samples it stands for, frames in a row that did not fit.
#[derive(Clone, Copy, Default)]
struct Learnt {
    l: [f64; P],
    n: f64,
    misfit: u32,
}

/// The learnt patterns, and the window of the last frame length.
#[derive(Default)]
pub struct Cores {
    learnt: HashMap<Key, Learnt>,
    win: Window,
}

/// A Hann window of `h.len()` samples and its sum per phase.
#[derive(Default)]
pub struct Window {
    h: Vec<f32>,
    wt: [f64; P],
}

impl Window {
    fn of(&mut self, n: usize) -> &Window {
        if self.h.len() != n {
            let d = 2.0 * std::f64::consts::PI / (n - 1) as f64;
            self.h.resize(n, 0.0);
            for (k, h) in self.h.iter_mut().enumerate() {
                *h = (0.5 - 0.5 * (d * k as f64).cos()) as f32;
            }
            self.wt = [0.0; P];
            for (k, &h) in self.h.iter().enumerate() {
                self.wt[k % P] += h as f64;
            }
        }
        self
    }
}

impl Cores {
    #[cfg(test)]
    /// Takes the core offsets off the words `w` of channel `key.1`. Returns the turn applied (word k had the offset
    /// of phase (k + turn) % 8 of the learnt pattern), None when the frame was left as it is.
    pub fn apply(&mut self, key: Key, w: &mut [u16]) -> Option<usize> {
        let correction = self.learn(key, w.len(), |i| w[i])?;
        for (i, x) in w.iter_mut().enumerate() {
            *x = correction.word(i, *x);
        }
        Some(correction.turn)
    }

    /// Learn on the acquisition thread; the returned value belongs to this frame alone.
    /// `sample` reads de-interleaved words without allocating a temporary channel vector.
    pub(super) fn learn(&mut self, key: Key, len: usize, sample: impl Fn(usize) -> u16) -> Option<Correction> {
        let (c, n) = estimate_at(len, sample, &mut self.win)?;
        let e = self.learnt.entry(key).or_default();
        if e.n == 0.0 || e.misfit >= MISFITS {
            // the first frame is the pattern
            *e = Learnt { l: c, n, misfit: 0 };
            return Some(Correction::new(&c, 0));
        }
        let (s, rest, ref_) = e.fit(&c);
        if rest >= ref_ {
            e.misfit += 1;
            return None;
        }
        e.misfit = 0;
        if rest < 0.5 * ref_ {
            let a = n / (e.n + n);
            for (p, &cp) in c.iter().enumerate() {
                let l = &mut e.l[(p + s) % P];
                *l += a * (cp - *l);
            }
            e.n = (e.n + n).min(MEMORY);
        }
        Some(Correction::new(&e.l, s))
    }

    /// Match with the pattern valid before this frame, without changing learning or misfit counts.
    pub(super) fn snapshot(&mut self, key: Key, len: usize, sample: impl Fn(usize) -> u16) -> Option<Correction> {
        let e = self.learnt.get(&key)?;
        let (c, _) = estimate_at(len, sample, &mut self.win)?;
        let (turn, rest, reference) = e.fit(&c);
        (rest < reference).then(|| Correction::new(&e.l, turn))
    }

    #[cfg(test)]
    /// The learnt pattern of `key` (codes, phase 0 first) and the samples it stands for.
    pub fn pattern(&self, key: Key) -> Option<([f64; P], f64)> {
        self.learnt.get(&key).map(|e| (e.l, e.n))
    }
}

impl Learnt {
    fn fit(&self, c: &[f64; P]) -> (usize, f64, f64) {
        let score = |s: usize| (0..P).map(|p| c[p] * self.l[(p + s) % P]).sum::<f64>();
        let turn = (0..P).max_by(|&a, &b| score(a).total_cmp(&score(b))).unwrap_or(0);
        // Compare unexplained energy with the smaller of pattern and estimate. Below half learns;
        // below all corrects only; above all leaves the frame unchanged (for example, core-frequency signals).
        let ll: f64 = self.l.iter().map(|x| x * x).sum();
        let cc: f64 = c.iter().map(|x| x * x).sum();
        let rest = (0..P).map(|p| (c[p] - self.l[(p + turn) % P]).powi(2)).sum();
        (turn, rest, ll.min(cc))
    }
}

/// Rounded offsets in sample order, copied into a batch job after ordered learning.
#[derive(Clone, Copy, Debug)]
pub(super) struct Correction {
    offsets: [i32; P],
    /// the turn `apply` reports to the tests
    #[cfg(test)]
    turn: usize,
}

impl Correction {
    fn new(pattern: &[f64; P], turn: usize) -> Self {
        Self {
            offsets: std::array::from_fn(|k| pattern[(k + turn) % P].round() as i32),
            #[cfg(test)]
            turn,
        }
    }

    pub(super) fn word(&self, i: usize, word: u16) -> u16 {
        (word as i32 - self.offsets[i % P]).clamp(0, 65535) as u16
    }
}

#[cfg(test)]
/// A frame's own estimate of the pattern: the Hann-weighted mean of each phase k % 8 of the first `LOOK` words, less
/// the mean of the 8 (a windowed DFT at the multiples of a 8th of the rate). With the samples it used; None for short
/// frames.
pub fn estimate(w: &[u16]) -> Option<([f64; P], f64)> {
    estimate_with(w, &mut Window::default())
}

#[cfg(test)]
/// `estimate` with a window kept from frame to frame: 8 words at a time, in f32 over blocks of 256 x 8 words.
fn estimate_with(w: &[u16], win: &mut Window) -> Option<([f64; P], f64)> {
    estimate_at(w.len(), |i| w[i], win)
}

fn estimate_at(len: usize, sample: impl Fn(usize) -> u16, win: &mut Window) -> Option<([f64; P], f64)> {
    let n = len.min(LOOK);
    if n < MIN {
        return None;
    }
    let win = win.of(n);
    let mut sum = [0.0f64; P];
    let (h8, ht) = win.h.as_chunks::<P>();
    let mut block = [0.0f32; P];
    for (i, h) in h8.iter().enumerate() {
        for p in 0..P {
            block[p] += h[p] * (sample(i * P + p) as f32 - 32768.0);
        }
        if i % 256 == 255 {
            for p in 0..P {
                sum[p] += block[p] as f64;
            }
            block = [0.0; P];
        }
    }
    for p in 0..P {
        sum[p] += block[p] as f64;
    }
    for (p, &h) in ht.iter().enumerate() {
        sum[p] += h as f64 * (sample(h8.len() * P + p) as f64 - 32768.0);
    }
    let mut c: [f64; P] = std::array::from_fn(|p| if win.wt[p] > 0.0 { sum[p] / win.wt[p] } else { 0.0 });
    let m = c.iter().sum::<f64>() / P as f64;
    for x in c.iter_mut() {
        *x -= m;
    }
    Some((c, n as f64))
}

#[cfg(test)]
mod tests {
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
        let ph = rng.next() * 6.283;
        (0..n)
            .map(|k| {
                (32768.0 + amp * (6.283185307 * cyc * k as f64 + ph).sin() + 90.0 * rng.gauss() + pat[(k + turn) % P])
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
}
