// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

use super::CorrectedCodes;

/// The samples of one channel as `deposit` needs them: their number, the extremes of a range and one sample.
pub trait Samples {
    fn len(&self) -> usize;
    /// (min, max) of samples `a..b` (`min_max`: NaN skipped; (inf, -inf) when none)
    fn extremes(&self, a: usize, b: usize) -> (f32, f32);
    fn at(&self, i: usize) -> f32;
}

impl Samples for [f32] {
    fn len(&self) -> usize {
        <[f32]>::len(self)
    }
    #[inline]
    fn extremes(&self, a: usize, b: usize) -> (f32, f32) {
        min_max(&self[a..b])
    }
    #[inline]
    fn at(&self, i: usize) -> f32 {
        self[i]
    }
}

/// One channel of a read-out (16-bit little-endian words, `stride` words per group, this channel's at `word`) with the
/// board's word-to-volt mapping. The mapping is monotone (a multiply by `scale` and a subtraction, each rounded
/// monotonically), so the extremes of the volts are the volts of the extreme words: exact, without converting every word.
#[derive(Clone, Copy)]
pub struct Codes<'a> {
    pub bytes: &'a [u8],
    /// the same bytes as words when they are 2-byte aligned (the read-out buffers are): the vector loops
    pub words: Option<&'a [u16]>,
    pub stride: usize,
    pub word: usize,
    pub n: usize,
    pub affine: (f32, f32),
    pub(crate) corrected: Option<CorrectedCodes>,
    pub volt: fn(u16, (f32, f32)) -> f32,
}

impl Samples for Codes<'_> {
    fn len(&self) -> usize {
        self.n
    }
    #[inline]
    fn extremes(&self, a: usize, b: usize) -> (f32, f32) {
        if a >= b {
            return (f32::INFINITY, f32::NEG_INFINITY);
        }
        let (lo, hi) = self.code_extremes(a, b);
        self.calibrated(lo, hi)
    }
    #[inline]
    fn at(&self, i: usize) -> f32 {
        self.volts(self.at_code(i))
    }
}

impl Codes<'_> {
    #[inline]
    pub(crate) fn code_extremes(&self, a: usize, b: usize) -> (u16, u16) {
        if self.corrected.is_some() {
            (a..b).map(|i| self.at_code(i)).fold((u16::MAX, u16::MIN), |(lo, hi), v| (lo.min(v), hi.max(v)))
        } else {
            match (self.words, self.stride) {
                (Some(w), s) => word_extremes(w, s, self.word, a, b),
                (None, 1) => code_extremes::<2>(&self.bytes[2 * a..2 * b], 0),
                (None, 2) => code_extremes::<4>(&self.bytes[4 * a..(4 * b).min(self.bytes.len())], self.word),
                (None, _) => code_extremes::<8>(&self.bytes[8 * a..(8 * b).min(self.bytes.len())], self.word),
            }
        }
    }
}

/// Smallest and largest of samples `a..b` (not empty) of the channel at `word` of every group of `stride` words.
/// On aarch64 NEON: whole groups of 8 samples through the de-interleaving loads (ld1/ld2/ld4), the rest one by one.
#[inline]
fn word_extremes(w: &[u16], stride: usize, word: usize, a: usize, b: usize) -> (u16, u16) {
    let (mut lo, mut hi) = (u16::MAX, u16::MIN);
    #[allow(unused_mut)] // Only the NEON loop advances i before the scalar remainder.
    let mut i = a;
    #[cfg(target_arch = "aarch64")]
    {
        use core::arch::aarch64::*;
        // SAFETY: NEON is aarch64 baseline; every load reads 8 groups that lie inside `w` (checked by `end`)
        unsafe {
            let (mut vl, mut vh) = (vdupq_n_u16(u16::MAX), vdupq_n_u16(0));
            let end = |i: usize| (i + 8) * stride <= w.len() && i + 8 <= b;
            match stride {
                1 => {
                    while end(i) {
                        let x = vld1q_u16(w.as_ptr().add(i));
                        (vl, vh) = (vminq_u16(vl, x), vmaxq_u16(vh, x));
                        i += 8;
                    }
                }
                2 => {
                    while end(i) {
                        let g = vld2q_u16(w.as_ptr().add(2 * i));
                        let x = if word == 0 { g.0 } else { g.1 };
                        (vl, vh) = (vminq_u16(vl, x), vmaxq_u16(vh, x));
                        i += 8;
                    }
                }
                4 => {
                    while end(i) {
                        let g = vld4q_u16(w.as_ptr().add(4 * i));
                        let x = match word {
                            0 => g.0,
                            1 => g.1,
                            2 => g.2,
                            _ => g.3,
                        };
                        (vl, vh) = (vminq_u16(vl, x), vmaxq_u16(vh, x));
                        i += 8;
                    }
                }
                _ => {}
            }
            (lo, hi) = (lo.min(vminvq_u16(vl)), hi.max(vmaxvq_u16(vh)));
        }
    }
    while i < b {
        let x = w[i * stride + word];
        lo = lo.min(x);
        hi = hi.max(x);
        i += 1;
    }
    (lo, hi)
}

/// Smallest and largest word `k` of every group of `G` bytes in `b` (the groups of one range of samples; the last
/// group may be short when its word is still inside).
#[inline]
#[allow(clippy::chunks_exact_to_as_chunks)] // Unit cost ratios (20261122-121730 / 091712): 1 ch 5 ns 0.95, 1 ch 50 ns 0.94, 4 ch 200 ns 0.92.
fn code_extremes<const G: usize>(b: &[u8], k: usize) -> (u16, u16) {
    let (mut lo, mut hi) = (u16::MAX, u16::MIN);
    let mut it = b.chunks_exact(G);
    for g in &mut it {
        let x = u16::from_le_bytes([g[2 * k], g[2 * k + 1]]);
        lo = lo.min(x);
        hi = hi.max(x);
    }
    let r = it.remainder();
    if r.len() >= 2 * k + 2 {
        let x = u16::from_le_bytes([r[2 * k], r[2 * k + 1]]);
        lo = lo.min(x);
        hi = hi.max(x);
    }
    (lo, hi)
}

/// Minimum and maximum of `s`, NaN skipped (`f32::min`/`max`: fminnm/fmaxnm on aarch64); (inf, -inf) when empty or
/// all NaN. On aarch64 four NEON accumulators of four lanes each (16 samples per step) instead of one compare chain
/// per sample, which bound the loop by the compare latency. The same values give the same extremes in any order, so
/// the result equals the one-chain loop (a zero may come back as -0.0 where the chain gave +0.0 or the other way;
/// both map to the same pixel row).
#[inline]
pub fn min_max(s: &[f32]) -> (f32, f32) {
    #[cfg(target_arch = "aarch64")]
    {
        // SAFETY: NEON is part of the aarch64 baseline; loads stay inside `s` (chunks of 16)
        unsafe { min_max_neon(s) }
    }
    #[cfg(not(target_arch = "aarch64"))]
    {
        min_max_scalar(s)
    }
}

/// `min_max` as one chain: the reference, and the code on other targets.
#[cfg(any(test, not(target_arch = "aarch64")))]
pub fn min_max_scalar(s: &[f32]) -> (f32, f32) {
    let (mut a, mut b) = (f32::INFINITY, f32::NEG_INFINITY);
    for &x in s {
        a = a.min(x);
        b = b.max(x);
    }
    (a, b)
}

#[cfg(target_arch = "aarch64")]
#[allow(clippy::chunks_exact_to_as_chunks)] // Unit cost ratios (20261122-121730 / 091712): 1 ch 5 ns 0.95, 1 ch 50 ns 0.94, 4 ch 200 ns 0.92.
#[inline]
// SAFETY: The caller guarantees NEON, and every load stays inside its 16-sample group.
unsafe fn min_max_neon(s: &[f32]) -> (f32, f32) {
    use core::arch::aarch64::*;
    let mut it = s.chunks_exact(16);
    let (mut n0, mut n1, mut n2, mut n3) = (
        vdupq_n_f32(f32::INFINITY),
        vdupq_n_f32(f32::INFINITY),
        vdupq_n_f32(f32::INFINITY),
        vdupq_n_f32(f32::INFINITY),
    );
    let (mut x0, mut x1, mut x2, mut x3) = (
        vdupq_n_f32(f32::NEG_INFINITY),
        vdupq_n_f32(f32::NEG_INFINITY),
        vdupq_n_f32(f32::NEG_INFINITY),
        vdupq_n_f32(f32::NEG_INFINITY),
    );
    for c in &mut it {
        let p = c.as_ptr();
        let (a, b, d, e) = (vld1q_f32(p), vld1q_f32(p.add(4)), vld1q_f32(p.add(8)), vld1q_f32(p.add(12)));
        n0 = vminnmq_f32(n0, a);
        n1 = vminnmq_f32(n1, b);
        n2 = vminnmq_f32(n2, d);
        n3 = vminnmq_f32(n3, e);
        x0 = vmaxnmq_f32(x0, a);
        x1 = vmaxnmq_f32(x1, b);
        x2 = vmaxnmq_f32(x2, d);
        x3 = vmaxnmq_f32(x3, e);
    }
    let mut a = vminnmvq_f32(vminnmq_f32(vminnmq_f32(n0, n1), vminnmq_f32(n2, n3)));
    let mut b = vmaxnmvq_f32(vmaxnmq_f32(vmaxnmq_f32(x0, x1), vmaxnmq_f32(x2, x3)));
    for &x in it.remainder() {
        a = a.min(x);
        b = b.max(x);
    }
    (a, b)
}

/// Max, min, sum and sum of squares of the finite samples of one channel of one frame: the voltage measurements of
/// the statistics (Max, Min, Pk-pk, Mean, RMS, Std dev) for every frame the fast path draws, not only the record.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Fold {
    pub n: u32,
    pub min: f32,
    pub max: f32,
    pub sum: f64,
    pub sumsq: f64,
}

impl Fold {
    pub fn of<S: Samples + ?Sized>(v: &S, range: std::ops::Range<usize>) -> Fold {
        let (mut mn, mut mx) = (f32::INFINITY, f32::NEG_INFINITY);
        // four partial sums in f32 per block of 4096, added up in f64: vector registers, no loss over long records
        let (mut sum, mut sumsq, mut n) = (0.0f64, 0.0f64, 0u32);
        for start in (range.start..range.end).step_by(4096) {
            let (mut s, mut q) = ([0.0f32; 4], [0.0f32; 4]);
            for (j, i) in (start..(start + 4096).min(range.end)).enumerate() {
                let x = v.at(i);
                if x.is_finite() {
                    mn = mn.min(x);
                    mx = mx.max(x);
                    s[j & 3] += x;
                    q[j & 3] += x * x;
                    n += 1;
                }
            }
            sum += s.iter().map(|&x| x as f64).sum::<f64>();
            sumsq += q.iter().map(|&x| x as f64).sum::<f64>();
        }
        Fold { n, min: mn, max: mx, sum, sumsq }
    }

    /// The value of measurement `k`, None for one it does not cover (or too few samples, as `measure::analyze`).
    pub fn get(&self, k: oos_dsp::measure::Meas) -> Option<f64> {
        use oos_dsp::measure::Meas;
        if self.n < 4 {
            return None;
        }
        let n = self.n as f64;
        let mean = self.sum / n;
        Some(match k {
            Meas::Vmax => self.max as f64,
            Meas::Vmin => self.min as f64,
            Meas::Vpp => self.max as f64 - self.min as f64,
            Meas::Vavg => mean,
            Meas::Vrms => (self.sumsq / n).sqrt(),
            Meas::StdDev | Meas::VrmsAc => (self.sumsq / n - mean * mean).max(0.0).sqrt(),
            _ => return None,
        })
    }

    /// Measurements `get` covers.
    pub fn covers(k: oos_dsp::measure::Meas) -> bool {
        use oos_dsp::measure::Meas;
        matches!(k, Meas::Vmax | Meas::Vmin | Meas::Vpp | Meas::Vavg | Meas::Vrms | Meas::StdDev | Meas::VrmsAc)
    }
}
