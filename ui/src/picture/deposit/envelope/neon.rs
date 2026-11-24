// SPDX-License-Identifier: GPL-2.0-or-later

use super::{joined, Correction, Extrema, PHASES};
use core::arch::aarch64::*;

const WORDS: usize = 8;
const RAW: u8 = 0;
const SIGNED: u8 = 1;
const WIDE: u8 = 2;

pub(super) fn reduce<const N: usize>(
    words: &[[u16; N]],
    correction: &Correction<N>,
    edges: &[usize],
    join: bool,
    out: &mut Vec<Extrema<N>>,
) {
    let mode = if !correction.active {
        RAW
    } else if correction.offsets.iter().flatten().all(|&offset| (-32767..=32768).contains(&offset)) {
        SIGNED
    } else {
        WIDE
    };
    let len = words.len();
    let mut pad = [[0; N]; WORDS];
    let words = if len < WORDS / N {
        pad[..len].copy_from_slice(words);
        &pad[..WORDS / N]
    } else {
        words
    };
    let masks = Masks::new(correction, mode);
    match mode {
        RAW => columns::<RAW, N>(words, len, edges, join, out, &masks),
        SIGNED => columns::<SIGNED, N>(words, len, edges, join, out, &masks),
        _ => columns::<WIDE, N>(words, len, edges, join, out, &masks),
    }
}

struct Masks<const N: usize> {
    offsets: [[u16; N]; 2 * PHASES],
    add: [[u16; N]; 2 * PHASES],
}

impl<const N: usize> Masks<N> {
    fn new(correction: &Correction<N>, mode: u8) -> Self {
        let phases =
            |convert: fn(i32) -> u16| std::array::from_fn(|phase| correction.offsets[phase % PHASES].map(convert));
        match mode {
            RAW => Self { offsets: [[0; N]; 2 * PHASES], add: [[0; N]; 2 * PHASES] },
            SIGNED => Self { offsets: phases(|offset| offset.wrapping_neg() as u16), add: [[0; N]; 2 * PHASES] },
            _ => Self {
                offsets: phases(|offset| offset.clamp(0, 65535) as u16),
                add: phases(|offset| offset.saturating_neg().clamp(0, 65535) as u16),
            },
        }
    }

    #[inline(always)]
    unsafe fn load<const MODE: u8>(&self, base: &[[u16; N]], start: usize) -> uint16x8_t {
        // SAFETY: NEON is baseline on aarch64. The caller selects a full vector in
        // the record. Both repeated phase arrays hold eight words from any starting phase.
        unsafe {
            let data = vld1q_u16(base.as_ptr().add(start).cast());
            if MODE == RAW {
                return data;
            }
            let phase = start % PHASES;
            let offset = vld1q_u16(self.offsets.as_ptr().add(phase).cast());
            if MODE == SIGNED {
                vsqaddq_u16(data, vreinterpretq_s16_u16(offset))
            } else {
                let add = vld1q_u16(self.add.as_ptr().add(phase).cast());
                vqaddq_u16(vqsubq_u16(data, offset), add)
            }
        }
    }
}

#[inline]
fn columns<const MODE: u8, const N: usize>(
    words: &[[u16; N]],
    len: usize,
    edges: &[usize],
    join: bool,
    out: &mut Vec<Extrema<N>>,
    masks: &Masks<N>,
) {
    let rows = WORDS / N;
    let last_load = words.len() - rows;
    out.extend(edges.array_windows().map(|&[start, end]| {
        let start = joined(start, end, join);
        let end = end.min(len);
        if start >= end {
            return Extrema::EMPTY;
        }
        // SAFETY: NEON is baseline on aarch64. Every load starts at or before last_load;
        // the final vector has enough words even when the input was padded on the stack.
        unsafe {
            let (low, high) = if end - start < rows {
                let base = start.min(last_load);
                let value = masks.load::<MODE>(words, base);
                let positions = vld1q_u16([0, 1, 2, 3, 4, 5, 6, 7].as_ptr());
                let offsets = vsubq_u16(positions, vdupq_n_u16(((start - base) * N) as u16));
                let keep = vcltq_u16(offsets, vdupq_n_u16(((end - start) * N) as u16));
                (vorrq_u16(value, vmvnq_u16(keep)), vandq_u16(value, keep))
            } else {
                let tail = end - rows;
                let mut low = masks.load::<MODE>(words, tail);
                let mut high = low;
                let mut index = start;
                while index < tail {
                    let value = masks.load::<MODE>(words, index);
                    low = vminq_u16(low, value);
                    high = vmaxq_u16(high, value);
                    index += rows;
                }
                (low, high)
            };
            horizontal(low, high)
        }
    }));
}

#[inline]
unsafe fn horizontal<const N: usize>(mut low: uint16x8_t, mut high: uint16x8_t) -> Extrema<N> {
    // SAFETY: NEON is baseline on aarch64. Rotations preserve physical lanes
    // and the destination arrays hold one full vector.
    unsafe {
        if N == 1 {
            return Extrema { low: [vminvq_u16(low); N], high: [vmaxvq_u16(high); N] };
        }
        low = vminq_u16(low, vextq_u16::<4>(low, low));
        high = vmaxq_u16(high, vextq_u16::<4>(high, high));
        if N == 2 {
            low = vminq_u16(low, vextq_u16::<2>(low, low));
            high = vmaxq_u16(high, vextq_u16::<2>(high, high));
        }
        let mut lo = [0; WORDS];
        let mut hi = [0; WORDS];
        vst1q_u16(lo.as_mut_ptr(), low);
        vst1q_u16(hi.as_mut_ptr(), high);
        Extrema { low: std::array::from_fn(|i| lo[i]), high: std::array::from_fn(|i| hi[i]) }
    }
}

pub(super) fn dense_edges<'a>(inverse: &'a [f64], phase: f64, len: usize, first: &mut Vec<usize>) -> &'a [f64] {
    if len > 1 << 53 {
        return inverse; // Clipping in binary64 requires an exactly representable length.
    }
    let (pairs, tail) = inverse.as_chunks::<2>();
    // SAFETY: NEON is baseline on aarch64. Each source and destination is two
    // 64-bit elements. FCVTPU rounds upward and saturates negative values and NaN to zero.
    unsafe {
        let phase = vdupq_n_f64(phase);
        let limit = vdupq_n_f64(len as f64);
        first.extend(pairs.iter().flat_map(|pair| {
            let sum = vaddq_f64(phase, vld1q_f64(pair.as_ptr()));
            let indices = vcvtpq_u64_f64(vminq_f64(sum, limit));
            let mut edges = [0usize; 2];
            vst1q_u64(edges.as_mut_ptr().cast(), indices);
            edges
        }));
    }
    tail
}
