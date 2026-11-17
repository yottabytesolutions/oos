// SPDX-License-Identifier: GPL-2.0-or-later

//! Allocation observer entry points for existing host kernels. Setup is outside the observer.
//! Later migration stages extend this list when their allocation-free paths land.

use crate::fastdraw::{self, Codes, Map};
use std::hint::black_box;

pub fn run(mut sample: impl FnMut(&str, &mut dyn FnMut())) {
    let (w, h, n) = (970, 448, 125_000);
    let samples: Vec<f32> = (0..n).map(|i| (i as f32 * 0.01).sin()).collect();
    let bytes: Vec<u8> = (0..n).flat_map(|i| (32768 + (i % 7500) as u16).to_le_bytes()).collect();
    let map = Map {
        ti: n as f64 / 2.0,
        x_per_sample: w as f64 / n as f64,
        x0: w as f64 / 2.0,
        y_per_volt: -56.0,
        y0: h as f32 / 2.0,
    };
    let mut pixels = vec![0u16; w * h];
    sample("deposit spans", &mut || fastdraw::deposit(&mut pixels, w, h, black_box(&samples), &map, 1));
    sample("deposit dots", &mut || fastdraw::deposit_dots(&mut pixels, w, h, black_box(&samples), &map, 1));
    let codes = Codes {
        bytes: &bytes,
        words: None,
        stride: 1,
        word: 0,
        n,
        affine: (1.0 / 7500.0, 32768.0 / 7500.0),
        volt: |x, (a, b)| x as f32 * a - b,
    };
    sample("deposit codes", &mut || fastdraw::deposit_codes(&mut pixels, w, h, black_box(&codes), &map, 1));
    crate::board::scripted::core_matching_probe(&mut sample);
    sample("cheap measurements", &mut || {
        black_box(crate::framestats::cheap_results(black_box(&samples)));
    });
}

mod deep_steps;
pub use deep_steps::run as deep_step;
