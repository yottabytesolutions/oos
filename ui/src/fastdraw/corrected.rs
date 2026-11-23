// SPDX-License-Identifier: GPL-2.0-or-later
//! Immutable per-record interleave correction and the original f64 calibrated conversion.

use crate::board::{correct_adc_code, word_volts_f64};

#[derive(Clone, Copy)]
pub(crate) struct CorrectedCodes {
    pub offsets: [i32; 8],
    pub scale: f64,
    pub offset: f64,
}
impl CorrectedCodes {
    #[inline]
    pub fn code(&self, index: usize, raw: u16) -> u16 {
        correct_adc_code(raw, self.offsets[index % 8])
    }
    #[inline]
    pub fn volts(&self, code: u16) -> f32 {
        word_volts_f64(code, self.scale, self.offset)
    }
}

impl super::Codes<'_> {
    /// Word `i` of the channel.
    #[inline]
    pub fn at_code(&self, i: usize) -> u16 {
        let j = i * self.stride + self.word;
        let raw = if let Some(words) = self.words {
            words[j]
        } else {
            u16::from_le_bytes([self.bytes[2 * j], self.bytes[2 * j + 1]])
        };
        self.corrected.map_or(raw, |c| c.code(i, raw))
    }
    #[inline]
    pub(crate) fn volts(&self, code: u16) -> f32 {
        self.corrected.map_or_else(|| (self.volt)(code, self.affine), |c| c.volts(code))
    }
    #[inline]
    pub(crate) fn calibrated(&self, lo: u16, hi: u16) -> (f32, f32) {
        let (x, y) = (self.volts(lo), self.volts(hi));
        if x <= y {
            (x, y)
        } else {
            (y, x)
        }
    }
}

#[cfg(test)]
mod tests;
