// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! CRC-32 of zlib, PNG, zip and IEEE 802.3 (reflected polynomial 0xEDB88320, preset and final XOR all ones).

/// CRC-32 tables for eight bytes per step ("slicing by 8"): `T[0]` is the usual byte table, `T[k][n]` the CRC of byte
/// `n` followed by `k` zero bytes.
const CRC_T: [[u32; 256]; 8] = {
    let mut t = [[0u32; 256]; 8];
    let mut n = 0;
    while n < 256 {
        let mut c = n as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 { 0xedb8_8320 ^ (c >> 1) } else { c >> 1 };
            k += 1;
        }
        t[0][n] = c;
        n += 1;
    }
    let mut k = 1;
    while k < 8 {
        let mut n = 0;
        while n < 256 {
            let c = t[k - 1][n];
            t[k][n] = t[0][(c & 0xff) as usize] ^ (c >> 8);
            n += 1;
        }
        k += 1;
    }
    t
};

/// CRC-32 (ISO 3309 / IEEE 802.3, as zlib.crc32).
pub fn crc32(data: &[u8]) -> u32 {
    let t = &CRC_T;
    let mut c = !0u32;
    let (blocks, rest) = data.as_chunks::<8>();
    for b in blocks {
        let lo = c ^ u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
        c = t[7][(lo & 0xff) as usize]
            ^ t[6][(lo >> 8 & 0xff) as usize]
            ^ t[5][(lo >> 16 & 0xff) as usize]
            ^ t[4][(lo >> 24) as usize]
            ^ t[3][b[4] as usize]
            ^ t[2][b[5] as usize]
            ^ t[1][b[6] as usize]
            ^ t[0][b[7] as usize];
    }
    !rest.iter().fold(c, |c, &b| t[0][((c ^ b as u32) & 0xff) as usize] ^ (c >> 8))
}

#[cfg(test)]
mod tests {
    use super::crc32;

    #[test]
    fn check_values() {
        assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
        assert_eq!(crc32(b"IEND"), 0xae42_6082);
        assert_eq!(crc32(&[]), 0);
    }

    #[test]
    fn eight_byte_steps_match_the_bitwise_definition() {
        let data: Vec<u8> = (0..20000u32).map(|i| (i.wrapping_mul(2654435761) >> 13) as u8).collect();
        for n in (0..70).chain([5551, 5552, 5553, 11104, 20000]) {
            let d = &data[..n];
            let bitwise = !d.iter().fold(!0u32, |mut c, &x| {
                c ^= x as u32;
                for _ in 0..8 {
                    c = if c & 1 != 0 { 0xedb8_8320 ^ (c >> 1) } else { c >> 1 };
                }
                c
            });
            assert_eq!(crc32(d), bitwise, "CRC of {} bytes", n);
        }
    }
}
