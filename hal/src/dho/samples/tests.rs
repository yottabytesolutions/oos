// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

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
