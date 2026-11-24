// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;
use crate::fastdraw::{Codes, Samples};
#[test]
fn per_index_clamped_correction_extrema_and_calibration_match_decoded_bits() {
    for stride in [1, 2, 4] {
        for lane in 0..stride {
            let bytes: Vec<u8> = (0..129 * stride)
                .flat_map(|i| {
                    let raw = match i % 11 {
                        0 => 0,
                        1 => 65535,
                        _ => (i * 17371) as u16,
                    };
                    raw.to_le_bytes()
                })
                .collect();
            for scale in [0.17, std::f64::consts::PI, -0.7] {
                let correction =
                    CorrectedCodes { offsets: [-65535, 65535, -80, 40, -20, 0, 73, -2], scale, offset: 0.031 };
                let codes = Codes {
                    bytes: &bytes,
                    words: None,
                    stride,
                    word: lane,
                    n: 129,
                    affine: (0., 0.),
                    volt: oos_hal::dho::seq::volt_code,
                    corrected: Some(correction),
                };
                let expected: Vec<f32> = (0..129)
                    .map(|i| {
                        let j = 2 * (i * stride + lane);
                        let raw = u16::from_le_bytes([bytes[j], bytes[j + 1]]);
                        let code = (raw as i32 - correction.offsets[i % 8]).clamp(0, 65535) as u16;
                        ((code as f64 - 32768.0) / 7500.0 * scale - 0.031) as f32
                    })
                    .collect();
                for (i, v) in expected.iter().enumerate() {
                    assert_eq!(codes.at(i).to_bits(), v.to_bits());
                }
                for a in 0..129 {
                    for b in a..=129 {
                        let want = expected[a..b]
                            .iter()
                            .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), v| (lo.min(*v), hi.max(*v)));
                        let got = codes.extremes(a, b);
                        assert_eq!(
                            (got.0.to_bits(), got.1.to_bits()),
                            (want.0.to_bits(), want.1.to_bits()),
                            "stride {stride} lane {lane} {a}..{b}"
                        );
                    }
                }
            }
        }
    }
}
