// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

use super::{record_geometry, tdiv_fs, time_fs, Settings};
use crate::dho::hz;
use std::num::NonZeroU64;

#[test]
fn times_survive_f32() {
    assert_eq!(time_fs(1e-6f32 as f64), 1_000_000_000);
    assert_eq!(time_fs(5e-3f32 as f64), 5_000_000_000_000);
    assert_eq!(time_fs(200e-3f32 as f64), 200_000_000_000_000);
    assert_eq!(time_fs(-2e-6f32 as f64), -2_000_000_000);
    assert_eq!(time_fs(8e-9), 8_000_000);
    assert_eq!(time_fs(1.23456e-6), 1_234_560_000);
    assert_eq!(time_fs(0.0), 0);
}

#[test]
fn f32_time_base_still_hits_the_rate_table() {
    assert_eq!(tdiv_fs(200e-6_f32 as f64), 200_000_000_000);
    assert_eq!(tdiv_fs(2e-9), 2_000_000);
    assert_eq!(tdiv_fs(500e-3_f32 as f64), 500_000_000_000_000);
    // 100 kpts at 200 us/div: 50 MSa/s is remapped to 31.25 MSa/s like the stock system does
    let p = hz::prep(1, tdiv_fs(200e-6_f32 as f64), 100_000, 0);
    assert_eq!(p.rate, 31_250_000);
}

#[test]
fn explicit_rate_requires_a_divider_and_an_aligned_record_that_fits() {
    for (rate, points) in
        [(703_120_000, 70_312), (14_062_400, 70_312), (12_500_000, 7), (12_500_000, 70_313), (12_500_000, 500_000_000)]
    {
        let s = Settings { sample_rate: NonZeroU64::new(rate), depth: points, ..Settings::default() };
        assert!(record_geometry(&s).is_err(), "{points} at {rate}");
    }
}
