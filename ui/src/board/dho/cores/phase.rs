// SPDX-License-Identifier: GPL-2.0-or-later

use super::{fold_period, Correction, Learnt, P};

/// A template folded to the number of distinct ADC cores visited by a lane.
#[derive(Clone, Copy)]
pub(in crate::board::dho) struct Matcher {
    offsets: [f64; P],
    filtered: [f64; 2 * P],
    energy: f64,
    period: usize,
}

impl Matcher {
    pub(in crate::board::dho) fn new(learnt: Learnt, period: usize) -> Option<Self> {
        let mut offsets = learnt.l;
        fold_period(&mut offsets, period);
        if offsets.iter().all(|offset| offset.abs() < 0.5) {
            return None;
        }
        let filtered = std::array::from_fn(|p| 2.0 * offsets[p % P] - offsets[(p + P - 1) % P] - offsets[(p + 1) % P]);
        let energy = filtered[..period].iter().map(|x| x * x).sum();
        Some(Self { offsets, filtered, energy, period })
    }

    pub(in crate::board::dho) fn correction(&self, words: &[[u8; 2]], lane: usize, ratio: usize) -> Option<Correction> {
        let measured = match ratio {
            1 => difference::<1>(words, lane, self.period),
            2 => difference::<2>(words, lane, self.period),
            4 => difference::<4>(words, lane, self.period),
            _ => unreachable!("one, two or four DMA words per sample"),
        }?;
        let (turn, score) = (0..self.period)
            .map(|turn| {
                let dot = measured[..self.period].iter().zip(&self.filtered[turn..]).map(|(a, b)| a * b).sum::<f64>();
                (turn, dot)
            })
            .max_by(|a, b| a.1.total_cmp(&b.1))?;
        let energy = measured[..self.period].iter().map(|x| x * x).sum::<f64>();
        let residual = energy + self.energy - 2.0 * score;
        (residual < energy.min(self.energy)).then(|| Correction::new(&self.offsets, turn))
    }
}

/// Fold the symmetric second difference into ADC phases. Summing each neighbour's
/// contribution only shifts the phase accumulators, with two endpoint adjustments.
fn difference<const RATIO: usize>(words: &[[u8; 2]], lane: usize, period: usize) -> Option<[f64; P]> {
    let limit = if period == 2 { 512 } else { 128 };
    let samples = words.as_chunks::<RATIO>().0;
    let end = (samples.len().min(limit).checked_sub(1)? / period) * period;
    let start = period;
    let interior = samples.get(start..end)?;
    let count = interior.len() / period;
    if count < 8 {
        return None;
    }
    let read = |group: &[[u8; 2]; RATIO]| i32::from(u16::from_le_bytes(group[lane]));
    let mut sum = [0; P];
    match period {
        2 => sum_phases::<RATIO, 2>(interior, lane, &mut sum),
        4 => sum_phases::<RATIO, 4>(interior, lane, &mut sum),
        _ => sum_phases::<RATIO, P>(interior, lane, &mut sum),
    }
    let before = read(&samples[start - 1]) - read(&samples[end - 1]);
    let after = read(&samples[end]) - read(&samples[start]);
    let mut result = [0.0; P];
    for p in 0..period {
        let neighbours = sum[(p + period - 1) % period] + sum[(p + 1) % period];
        let boundary = if p == 0 {
            before
        } else if p + 1 == period {
            after
        } else {
            0
        };
        result[p] = f64::from(2 * sum[p] - neighbours - boundary) / count as f64;
    }
    // Endpoint adjustments can leave a DC component, which carries no turn information.
    let dc = result[..period].iter().sum::<f64>() / period as f64;
    for x in &mut result[..period] {
        *x -= dc;
    }
    Some(result)
}

fn sum_phases<const RATIO: usize, const PERIOD: usize>(samples: &[[[u8; 2]; RATIO]], lane: usize, sum: &mut [i32; P]) {
    for group in samples.as_chunks::<PERIOD>().0 {
        for p in 0..PERIOD {
            sum[p] += i32::from(u16::from_le_bytes(group[p][lane]));
        }
    }
}
