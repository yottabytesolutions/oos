// SPDX-License-Identifier: GPL-2.0-or-later

//! Fixed-point Lanczos evaluation for the screen and zoom placement grids.

pub(crate) type Taps = [i16; 16];

/// Visit consecutive vertices that can all borrow the same window of input samples.
/// The caller clips the grid to the record and supplies a nonnegative start and positive step.
#[inline]
pub(crate) fn fill_sample_runs<const BITS: u32>(
    out: &mut Vec<i32>,
    (mut x, step, count): (i64, i64, usize),
    mut fill: impl FnMut(usize, i64, &mut [i32]),
) {
    const { assert!(BITS < 63) };
    debug_assert!(x >= 0 && step > 0);
    let unit = 1u64 << BITS;
    out.resize(count, 0);
    let mut rest = out.as_mut_slice();
    while !rest.is_empty() {
        let to_next_sample = unit - (x as u64 & (unit - 1));
        let n = to_next_sample.div_ceil(step as u64).min(rest.len() as u64) as usize;
        let (run, tail) = rest.split_at_mut(n);
        fill((x >> BITS) as usize, x, run);
        if !tail.is_empty() {
            x += step * n as i64;
        }
        rest = tail;
    }
}

#[inline]
#[cfg(target_arch = "aarch64")]
pub(crate) fn dot(samples: &Taps, weights: &Taps) -> i32 {
    use core::arch::aarch64::*;
    // SAFETY: aarch64 has NEON and each array contains two full eight-element vectors.
    unsafe {
        let a = vld1q_s16(samples.as_ptr());
        let b = vld1q_s16(weights.as_ptr());
        let c = vld1q_s16(samples.as_ptr().add(8));
        let d = vld1q_s16(weights.as_ptr().add(8));
        let ab = vmlal_high_s16(vmull_s16(vget_low_s16(a), vget_low_s16(b)), a, b);
        let cd = vmlal_high_s16(vmull_s16(vget_low_s16(c), vget_low_s16(d)), c, d);
        vaddvq_s32(vaddq_s32(ab, cd))
    }
}

#[cfg(not(target_arch = "aarch64"))]
pub(crate) use scalar_dot as dot;

#[inline]
#[cfg(any(test, not(target_arch = "aarch64")))]
pub(crate) fn scalar_dot(samples: &Taps, weights: &Taps) -> i32 {
    samples.iter().zip(weights).map(|(&sample, &weight)| i32::from(sample) * i32::from(weight)).sum()
}
