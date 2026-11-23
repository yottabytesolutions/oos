// SPDX-License-Identifier: GPL-2.0-or-later
//! Sample arithmetic provided by the active board.

use super::Active;

/// Apply the active board's ADC core offset to one sample word.
#[inline]
pub(crate) fn correct_adc_code(raw: u16, offset: i32) -> u16 {
    Active::correct_adc_code(raw, offset)
}

/// Calibrate one sample word using the active board's f64 conversion.
#[inline]
pub(crate) fn word_volts_f64(code: u16, scale: f64, offset: f64) -> f32 {
    Active::word_volts_f64(code, scale, offset)
}
