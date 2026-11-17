// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! SHA-1 for the stock loader's image id, and streaming SHA-256/SHA-512 (FIPS 180-4).

use sha2::Digest;

/// SHA-256 state.
#[derive(Clone, Debug, Default)]
pub struct Sha256(sha2::Sha256);

impl Sha256 {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update(&mut self, data: &[u8]) {
        self.0.update(data);
    }

    pub fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }
}

/// SHA-256 of one buffer.
pub fn sha256(data: &[u8]) -> [u8; 32] {
    sha2::Sha256::digest(data).into()
}

/// SHA-512 state.
#[derive(Clone, Debug, Default)]
pub struct Sha512(sha2::Sha512);

impl Sha512 {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update(&mut self, data: &[u8]) {
        self.0.update(data);
    }

    pub fn finish(self) -> [u8; 64] {
        self.0.finalize().into()
    }
}

/// SHA-512 of one buffer.
pub fn sha512(data: &[u8]) -> [u8; 64] {
    sha2::Sha512::digest(data).into()
}

/// SHA-1 of the given parts in order (the stock loader's boot image id; not used for any security decision).
pub fn sha1(parts: &[&[u8]]) -> [u8; 20] {
    let mut h: [u32; 5] = [0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476, 0xc3d2e1f0];
    let total: u64 = parts.iter().map(|p| p.len() as u64).sum();
    let mut tail = [0u8; 64 + 72];
    let mut buf = [0u8; 64];
    let mut n = 0usize;
    let block = |h: &mut [u32; 5], b: &[u8]| {
        let mut w = [0u32; 80];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([b[4 * i], b[4 * i + 1], b[4 * i + 2], b[4 * i + 3]]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let [mut a, mut b_, mut c, mut d, mut e] = *h;
        for (i, wi) in w.iter().enumerate() {
            let (f, k) = match i / 20 {
                0 => ((b_ & c) | (!b_ & d), 0x5a827999),
                1 => (b_ ^ c ^ d, 0x6ed9eba1),
                2 => ((b_ & c) | (b_ & d) | (c & d), 0x8f1bbcdc),
                _ => (b_ ^ c ^ d, 0xca62c1d6u32),
            };
            let t = a.rotate_left(5).wrapping_add(f).wrapping_add(e).wrapping_add(k).wrapping_add(*wi);
            e = d;
            d = c;
            c = b_.rotate_left(30);
            b_ = a;
            a = t;
        }
        for (x, y) in h.iter_mut().zip([a, b_, c, d, e]) {
            *x = x.wrapping_add(y);
        }
    };
    for p in parts {
        for &x in p.iter() {
            buf[n] = x;
            n += 1;
            if n == 64 {
                block(&mut h, &buf);
                n = 0;
            }
        }
    }
    tail[..n].copy_from_slice(&buf[..n]);
    tail[n] = 0x80;
    let len = if n < 56 { 64 } else { 128 };
    tail[len - 8..len].copy_from_slice(&(total * 8).to_be_bytes());
    for c in tail[..len].chunks(64) {
        block(&mut h, c);
    }
    let mut out = [0u8; 20];
    for (i, v) in h.iter().enumerate() {
        out[4 * i..4 * i + 4].copy_from_slice(&v.to_be_bytes());
    }
    out
}

/// Lower-case hex of a byte string.
pub fn hex(b: &[u8]) -> String {
    let mut s = String::with_capacity(2 * b.len());
    for x in b {
        s.push_str(&format!("{:02x}", x));
    }
    s
}

/// Bytes from hex (even length, no separators); None on anything else.
pub fn unhex(s: &str) -> Option<Vec<u8>> {
    let s = s.trim();
    if s.len() % 2 != 0 {
        return None;
    }
    (0..s.len() / 2).map(|i| u8::from_str_radix(s.get(2 * i..2 * i + 2)?, 16).ok()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fips_examples() {
        assert_eq!(hex(&sha256(b"abc")), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert_eq!(hex(&sha256(b"")), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
        assert_eq!(
            hex(&sha256(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq")),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
        assert_eq!(
            hex(&sha512(b"abc")),
            "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a\
             2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f"
        );
        assert_eq!(
            hex(&sha512(b"")),
            "cf83e1357eefb8bdf1542850d66d8007d620e4050b5715dc83f4a921d36ce9ce\
             47d0d13c5d85f2b0ff8318d2877eec2f63b931bd47417a81a538327af927da3e"
        );
    }

    #[test]
    fn streaming_matches_one_shot() {
        let data: Vec<u8> = (0..1000u32).map(|i| (i * 7 + 3) as u8).collect();
        for cut in [0, 1, 55, 56, 63, 64, 65, 111, 112, 127, 128, 129, 500, 1000] {
            let mut a = Sha256::new();
            a.update(&data[..cut]);
            a.update(&data[cut..]);
            assert_eq!(a.finish(), sha256(&data), "sha256 cut {}", cut);
            let mut b = Sha512::new();
            for c in data[..cut].chunks(3) {
                b.update(c);
            }
            b.update(&data[cut..]);
            assert_eq!(b.finish(), sha512(&data), "sha512 cut {}", cut);
        }
        // a million 'a' (FIPS 180-2 example 3)
        let mut a = Sha256::new();
        let m = [b'a'; 1000];
        for _ in 0..1000 {
            a.update(&m);
        }
        assert_eq!(hex(&a.finish()), "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0");
    }

    #[test]
    fn sha1_examples() {
        assert_eq!(hex(&sha1(&[b"abc"])), "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert_eq!(hex(&sha1(&[b""])), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
        assert_eq!(
            hex(&sha1(&[b"abcdbcdecdefdefgefghfghighij", b"hijkijkljklmklmnlmnomnopnopq"])),
            "84983e441c3bd26ebaae4aa1f95129e5e54670f1"
        );
    }

    #[test]
    fn hex_round_trip() {
        assert_eq!(unhex("00ff10"), Some(vec![0, 255, 16]));
        assert_eq!(unhex("0"), None);
        assert_eq!(unhex("zz"), None);
        assert_eq!(hex(&[0xab, 1]), "ab01");
    }
}
