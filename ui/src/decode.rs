// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! Decoder settings compatibility exports.

pub use oos_core::decode::*;

/// C-PHY symbol rates of the decoder; 0 = from the signal.
pub const CPHY_RATES: [f64; 19] =
    [0.0, 0.5e9, 1e9, 1.5e9, 2e9, 2.25e9, 2.5e9, 3e9, 3.5e9, 4e9, 4.5e9, 6e9, 8e9, 10e9, 12e9, 16e9, 20e9, 25e9, 32e9];
pub fn cphy_rate_label(rate: f64) -> String {
    if rate == 0.0 {
        "Auto".into()
    } else {
        format!("{} Gsym/s", rate / 1e9)
    }
}
pub fn parse_cphy_rate(text: &str) -> Result<f64, String> {
    if text.trim().eq_ignore_ascii_case("AUTO") {
        return Ok(0.0);
    }
    let rate = text.trim().parse::<f64>().map_err(|_| "C-PHY symbol rate needs Hz or AUTO".to_string())?;
    if !rate.is_finite() || !(0.0..=1e12).contains(&rate) {
        return Err("C-PHY symbol rate must be AUTO or 0..1e12 sym/s".into());
    }
    Ok(rate)
}
