// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

use super::test_support::noise;
use super::volts_f64 as volts;
use super::*;

// Break it catches: a code map that puts a channel at another word than the decode, or volts that differ from it
#[test]
fn code_map_reads_what_the_decode_reads() {
    let cal = [(1.0, 0.1), (0.5, -0.2), (2.0, 0.0), (0.2, 0.05)];
    for mask in 1u32..16 {
        let bytes: Vec<u8> = (0..4 * 37 + 3u32).flat_map(|i| ((30_000 + i * 97 % 5000) as u16).to_le_bytes()).collect();
        let d = decode_volts(&bytes, mask, cal);
        let m = code_map(mask, cal);
        for (c, decoded) in d.iter().enumerate() {
            let Some(k) = m.word[c] else {
                assert!(decoded.is_empty(), "mask {} CH{}", mask, c + 1);
                continue;
            };
            let n = (bytes.len() / 2 + m.ratio - 1 - k) / m.ratio;
            let v: Vec<f32> = (0..n)
                .map(|i| {
                    volt_code(
                        u16::from_le_bytes([bytes[2 * (i * m.ratio + k)], bytes[2 * (i * m.ratio + k) + 1]]),
                        m.affine[c],
                    )
                })
                .collect();
            assert_eq!(&v, decoded, "mask {} CH{}", mask, c + 1);
        }
    }
}

#[test]
fn deinterleave_and_volts() {
    let words: Vec<u16> = (0..16).collect();
    let b: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
    let d = deinterleave(&b, 0b1010);
    assert_eq!(d[1], [0, 2, 4, 6, 8, 10, 12, 14]);
    assert_eq!(d[3], [1, 3, 5, 7, 9, 11, 13, 15]);
    let d = deinterleave(&b, 0b0111); // 3 channels run in 4-channel mode
    assert_eq!(d[2], [2, 6, 10, 14]);
    assert!(d[3].is_empty());
    // the replayed frame: 21518 = yref + yorg at 1 V/div, offset -1.5 V -> 0 V
    assert!((volts(&[21518], 1.0, -1.5)[0]).abs() < 1e-6);
    assert!((volts(&[32768 + 7500], 2.0, 0.0)[0] - 2.0).abs() < 1e-6);
}

/// The bound of `volt32` against the f64 `volt` of code `x`: 4 f32 ulps of the larger of the two terms.
fn f32_bound(x: u16, (scale, offset): (f64, f64)) -> f64 {
    4.0 * f32::EPSILON as f64 * ((x as f64 - 32768.0).abs() / 7500.0 * scale.abs() + offset.abs())
        + f32::MIN_POSITIVE as f64
}

// Break it catches: a fused decode that maps a word to the wrong channel, drops the words of an incomplete last
// group, or moves a value by more than the f32 rounding (the display and the measurements would move).
#[test]
fn fused_decode_equals_deinterleave_and_volts() {
    let cal = [(1.0, -1.5), (0.02, 0.013), (5.0, 2.5), (0.5, 0.0)];
    let mut worst = 0f64;
    for mask in 1..16u32 {
        for len in [0usize, 2, 6, 8, 14, 16, 2816, 8000, 12_502] {
            let b = noise(len, mask * 7919 + len as u32);
            let w = deinterleave(&b, mask);
            let got = decode_volts(&b, mask, cal);
            for c in 0..4 {
                assert_eq!(got[c].len(), w[c].len(), "mask {:#x}, {} bytes, CH{}", mask, len, c + 1);
                let want = volts(&w[c], cal[c].0, cal[c].1);
                for (i, (&g, &x)) in got[c].iter().zip(&w[c]).enumerate() {
                    let d = (g as f64 - want[i] as f64).abs();
                    worst = worst.max(d / (cal[c].0 / 7500.0));
                    assert!(
                        d <= f32_bound(x, cal[c]),
                        "mask {:#x}, {} bytes, CH{} word {}: {} vs {}",
                        mask,
                        len,
                        c + 1,
                        i,
                        g,
                        want[i]
                    );
                }
            }
        }
    }
    // in codes: far below one
    assert!(worst < 0.01, "{} codes", worst);
}

// Break it catches: kept vectors that keep stale samples or a channel that is no longer enabled, or that differ from
// the allocating decode.
#[test]
fn decode_into_kept_vectors_equals_decode() {
    let cal = [(1.0, -1.5), (0.02, 0.013), (5.0, 2.5), (0.5, 0.0)];
    let mut out: [Vec<f32>; 4] = Default::default();
    for (mask, len) in [(0xfu32, 8000usize), (1, 2816), (0b1010, 4002), (0b0111, 802), (8, 6)] {
        let b = noise(len, mask);
        decode_volts_into(&b, mask, cal, &mut out);
        let want = decode_volts(&b, mask, cal);
        for c in 0..4 {
            let bits = |v: &[f32]| v.iter().map(|x| x.to_bits()).collect::<Vec<u32>>();
            assert_eq!(bits(&out[c]), bits(&want[c]), "mask {:#x}, CH{}", mask, c + 1);
        }
    }
}

#[test]
fn both_precision_paths_keep_their_original_bits() {
    let words: Vec<u16> = (u16::MIN..=u16::MAX).collect();
    let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
    for (scale, offset) in [(1.0, -1.5), (0.02, 0.013), (5.0, 2.5), (0.5, 0.0)] {
        let roll = volts_f64(&words, scale, offset);
        let decoded = decode_volts(&bytes, 1, [(scale, offset); 4]);
        let a = (scale / 7500.0) as f32;
        let off = offset as f32;
        for (i, &x) in words.iter().enumerate() {
            let original_f64 = ((x as f64 - 32768.0) / 7500.0 * scale - offset) as f32;
            let original_f32 = (x as i32 - 32768) as f32 * a - off;
            assert_eq!(roll[i].to_bits(), original_f64.to_bits());
            assert_eq!(decoded[0][i].to_bits(), original_f32.to_bits());
        }
    }
}

#[test]
fn roll_keeps_the_distinct_f64_rounding() {
    let roll = volts_f64(&[1], 1.0, -1.5)[0];
    let decoded = decode_volts(&1u16.to_le_bytes(), 1, [(1.0, -1.5); 4])[0][0];
    assert_eq!(roll.to_bits(), 3_224_870_043);
    assert_eq!(decoded.to_bits(), 3_224_870_042);
}
