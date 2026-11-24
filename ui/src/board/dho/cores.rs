// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! Matching the ADC cores in software. Eight interleaved cores take turns on the samples, and each has an offset of its
//! own: on the DHO924S (2026-11-14) up to +-90 codes of the 16-bit words (7500 a division) after the stored
//! calibration, a fixed pattern that repeats every 8 samples at the full rate (a spur at 156.25 MHz, -52 dBc of a
//! 1 Vpp sine at 0.2 V/div), every 4 with two channels and every 2 with four, or when every 2nd or 4th sample is kept.
//! The pattern is the same at every V/div, but where it starts in a frame changes from frame to frame (it follows
//! the trigger point), so it is found in each frame.
//!
//! Learn offsets from one Hann-weighted record per batch. The remaining records use a bounded
//! phase estimate against that pattern. Core gains differ too and are left to calibration.

use oos_hal::dho::samples::correct_adc_code;
use std::collections::HashMap;

mod phase;
pub(super) use phase::Matcher;

#[cfg(any(target_os = "linux", test))]
mod fixture;
#[cfg(any(target_os = "linux", test))]
pub(super) use fixture::Pattern;

/// Phases of the pattern: the cores at the full rate.
pub const P: usize = 8;
/// Samples of the teaching record used to learn offsets.
const LEARN_LOOK: usize = 4_096;
/// The legacy whole-record estimator used as a fixture oracle.
#[cfg(test)]
const LOOK: usize = 1 << 16;
/// Samples a frame needs to take part.
const MIN: usize = 128;
/// Samples the learnt pattern stands for at most: a frame of n samples moves it by n / (this + n).
const MEMORY: f64 = (1 << 19) as f64;

/// Frames in a row that do not fit the learnt pattern before it is learnt again from the next one (a pattern learnt
/// from a bad first frame, a calibration meanwhile).
const MISFITS: u32 = 16;

/// What a pattern is learnt for: channel mask, channel, sample rate.
pub type Key = (u32, usize, u64);

/// A learnt pattern: offsets (codes, phase 0 first), the samples it stands for, frames in a row that did not fit.
#[derive(Clone, Copy, Default)]
pub(super) struct Learnt {
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
struct Window {
    h: Vec<f32>,
    wt: [f64; P],
}

type Estimate = ([f64; P], f64);

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
        let estimate = estimate_at(w.len().min(LOOK), |i| w[i], &mut self.win)?;
        let correction = self.fold(key, estimate)?;
        for (i, x) in w.iter_mut().enumerate() {
            *x = correction.word(i, *x);
        }
        Some(correction.turn)
    }

    fn fold(&mut self, key: Key, (c, n): Estimate) -> Option<Correction> {
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

    pub(super) fn learnt(&self, key: Key) -> Option<Learnt> {
        self.learnt.get(&key).copied()
    }

    pub(super) fn learn(
        &mut self,
        key: Key,
        len: usize,
        period: usize,
        sample: impl Fn(usize) -> u16,
    ) -> Option<Correction> {
        let (mut estimate, n) = estimate_at(len.min(LEARN_LOOK), sample, &mut self.win)?;
        fold_period(&mut estimate, period);
        if estimate.iter().all(|offset| offset.abs() < 0.5) {
            return None;
        }
        self.fold(key, (estimate, n))
    }

    #[cfg(test)]
    /// The learnt pattern of `key` (codes, phase 0 first) and the samples it stands for.
    pub fn pattern(&self, key: Key) -> Option<([f64; P], f64)> {
        self.learnt.get(&key).map(|e| (e.l, e.n))
    }
}

impl Learnt {
    #[cfg(any(target_os = "linux", test))]
    pub(super) fn correction(self) -> Correction {
        Correction::new(&self.l, 0)
    }

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
    pub(super) fn new(pattern: &[f64; P], turn: usize) -> Self {
        Self {
            offsets: std::array::from_fn(|k| pattern[(k + turn) % P].round() as i32),
            #[cfg(test)]
            turn,
        }
    }

    pub(super) fn offsets(self) -> [i32; P] {
        self.offsets
    }

    pub(super) fn word(&self, i: usize, word: u16) -> u16 {
        correct_adc_code(word, self.offsets[i % P])
    }
}

#[cfg(test)]
/// A frame's own estimate of the pattern: the Hann-weighted mean of each phase k % 8 of the first `LOOK` words, less
/// the mean of the 8 (a windowed DFT at the multiples of a 8th of the rate). With the samples it used; None for short
/// frames.
pub fn estimate(w: &[u16]) -> Option<([f64; P], f64)> {
    estimate_at(w.len().min(LOOK), |i| w[i], &mut Window::default())
}

fn estimate_at(n: usize, sample: impl Fn(usize) -> u16, win: &mut Window) -> Option<Estimate> {
    if n < MIN {
        return None;
    }
    let groups = (0..n / P).map(|i| std::array::from_fn(|p| f32::from(sample(i * P + p)) - 32768.0));
    Some(weighted(n, groups, |p| sample(n / P * P + p), win.of(n)))
}

fn fold_period(offsets: &mut [f64; P], period: usize) {
    let reduced: [f64; P] =
        std::array::from_fn(|p| offsets.iter().skip(p % period).step_by(period).sum::<f64>() / (P / period) as f64);
    *offsets = reduced;
}

fn weighted(n: usize, samples: impl Iterator<Item = [f32; P]>, tail: impl Fn(usize) -> u16, win: &Window) -> Estimate {
    debug_assert_eq!(win.h.len(), n);
    let mut sum = [0.0f64; P];
    let (groups, rest) = win.h.as_chunks::<P>();
    let mut block = [0.0f32; P];
    for (i, (samples, weights)) in samples.zip(groups).enumerate() {
        for p in 0..P {
            block[p] += weights[p] * samples[p];
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
    for (p, &h) in rest.iter().enumerate() {
        sum[p] += h as f64 * (tail(p) as f64 - 32768.0);
    }
    normalize(sum, win, n)
}

fn normalize(sum: [f64; P], win: &Window, n: usize) -> Estimate {
    let mut c: [f64; P] = std::array::from_fn(|p| if win.wt[p] > 0.0 { sum[p] / win.wt[p] } else { 0.0 });
    let mean = c.iter().sum::<f64>() / P as f64;
    for x in &mut c {
        *x -= mean;
    }
    (c, n as f64)
}

#[cfg(test)]
mod tests;
