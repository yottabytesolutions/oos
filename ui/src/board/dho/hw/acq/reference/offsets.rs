// SPDX-License-Identifier: GPL-2.0-or-later

//! ADC offsets of stored Peak extrema after the FPGA has selected the two words.

use crate::board::dho::cores::{Cores, P};
use oos_hal::dho::{
    hz,
    samples::correct_adc_code,
    seq::{self, Settings},
};

/// Returns whether any enabled lane lacked a learnt native-rate correction.
pub(super) fn correct(bytes: &mut [u8], frame_bytes: usize, s: &Settings, p: &hz::Params, cores: &Cores) -> bool {
    let rate = hz::CLOCK as u64 / u64::from(p.ratio);
    let period = P / p.ratio as usize;
    let block = (2 * (rate / p.rate) as usize).min(period);
    let map = seq::code_map(s.mask, s.ch.map(|channel| (channel.scale, channel.offset)));
    let offsets = std::array::from_fn::<_, 4, _>(|c| {
        let word = map.word[c]?;
        let learnt = cores.learnt((s.mask, c, rate))?;
        let pattern = learnt.correction().offsets();
        Some((word * 2, pair_offsets(&pattern[..period], block)))
    });
    let uncorrected = map.word.iter().zip(&offsets).any(|(word, offset)| word.is_some() && offset.is_none());
    let active = offsets.iter().flatten().filter(|(_, offsets)| *offsets != [0, 0]);
    let header = if s.header { 16 } else { 0 };
    let stride = map.ratio * 2;
    for (lane, [lo, hi]) in active.copied() {
        for frame in bytes.chunks_exact_mut(frame_bytes) {
            for pair in frame[header..].chunks_exact_mut(2 * stride) {
                let a = u16::from_le_bytes([pair[lane], pair[lane + 1]]);
                let b = u16::from_le_bytes([pair[lane + stride], pair[lane + stride + 1]]);
                // The word order is immaterial to the envelope. Apply the offsets by rank,
                // while preserving each word's position in the pair.
                let (first, second) = if a <= b { (lo, hi) } else { (hi, lo) };
                pair[lane..lane + 2].copy_from_slice(&correct_adc_code(a, first).to_le_bytes());
                pair[lane + stride..lane + stride + 2].copy_from_slice(&correct_adc_code(b, second).to_le_bytes());
            }
        }
    }
    uncorrected
}

fn pair_offsets(pattern: &[i32], block: usize) -> [i32; 2] {
    // On a flat input the extrema carry the smallest and largest offsets of the cores
    // in the block. If a block spans fewer cores, its starting phase is unknown.
    // The midpoint of the possible minima/maxima minimizes worst-case flat-input error.
    if block == pattern.len() {
        return [pattern.iter().copied().min().unwrap_or(0), pattern.iter().copied().max().unwrap_or(0)];
    }
    let mut low = [i32::MAX; 2];
    let mut high = [i32::MIN; 2];
    for start in 0..pattern.len() {
        let words = (0..block).map(|i| pattern[(start + i) % pattern.len()]);
        let extrema = [words.clone().min().unwrap_or(0), words.max().unwrap_or(0)];
        for (lane, value) in extrema.into_iter().enumerate() {
            low[lane] = low[lane].min(value);
            high[lane] = high[lane].max(value);
        }
    }
    std::array::from_fn(|lane| low[lane].midpoint(high[lane]))
}

#[cfg(test)]
mod tests;
