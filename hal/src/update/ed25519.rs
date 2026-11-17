// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! Ed25519 signatures (RFC 8032) for update packages.

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};

/// Public key of a 32-byte secret seed.
pub fn public_key(seed: &[u8; 32]) -> [u8; 32] {
    SigningKey::from_bytes(seed).verifying_key().to_bytes()
}

/// Signature of `msg` with the secret seed.
pub fn sign(seed: &[u8; 32], msg: &[u8]) -> [u8; 64] {
    SigningKey::from_bytes(seed).sign(msg).to_bytes()
}

/// True when `sig` is a strict Ed25519 signature of `msg` under `pk` (including S < L).
pub fn verify(pk: &[u8; 32], msg: &[u8], sig: &[u8; 64]) -> bool {
    let Ok(key) = VerifyingKey::from_bytes(pk) else { return false };
    key.verify_strict(msg, &Signature::from_bytes(sig)).is_ok()
}

#[cfg(test)]
mod tests {
    use super::super::hash::{hex, unhex};
    use super::*;

    const L: [u64; 4] = [0x5812631a5cf5d3ed, 0x14def9dea2f79cd6, 0, 0x1000000000000000];

    fn words<const N: usize>(b: &[u8]) -> [u64; N] {
        let mut w = [0u64; N];
        for (i, x) in w.iter_mut().enumerate() {
            let mut q = [0u8; 8];
            q.copy_from_slice(&b[8 * i..8 * i + 8]);
            *x = u64::from_le_bytes(q);
        }
        w
    }

    fn arr<const N: usize>(s: &str) -> [u8; N] {
        unhex(s).unwrap().try_into().unwrap()
    }

    // RFC 8032 7.1, TEST 1..3 and TEST SHA(abc)
    const VECTORS: [(&str, &str, &str, &str); 4] = [
        (
            "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60",
            "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a",
            "",
            "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e065224901555fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b",
        ),
        (
            "4ccd089b28ff96da9db6c346ec114e0f5b8a319f35aba624da8cf6ed4fb8a6fb",
            "3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c",
            "72",
            "92a009a9f0d4cab8720e820b5f642540a2b27b5416503f8fb3762223ebdb69da085ac1e43e15996e458f3613d0f11d8c387b2eaeb4302aeeb00d291612bb0c00",
        ),
        (
            "c5aa8df43f9f837bedb7442f31dcb7b166d38535076f094b85ce3a2e0b4458f7",
            "fc51cd8e6218a1a38da47ed00230f0580816ed13ba3303ac5deb911548908025",
            "af82",
            "6291d657deec24024827e69c3abe01a30ce548a284743a445e3680d7db5ac3ac18ff9b538d16f290ae67f760984dc6594a7c15e9716ed28dc027beceea1ec40a",
        ),
        (
            "833fe62409237b9d62ec77587520911e9a759cec1d19755b7da901b96dca3d42",
            "ec172b93ad5e563bf4932c70e1245034c35467ef2efd4d64ebf819683467e2bf",
            "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f",
            "dc2a4459e7369633a52b1bf277839a00201009a3efbf3ecb69bea2186c26b58909351fc9ac90b3ecfdfbc7c66431e0303dca179c138ac17ad9bef1177331a704",
        ),
    ];

    #[test]
    fn rfc8032_vectors() {
        for (sk, pk, msg, sig) in VECTORS {
            let sk: [u8; 32] = arr(sk);
            let pk: [u8; 32] = arr(pk);
            let msg = unhex(msg).unwrap();
            let sig: [u8; 64] = arr(sig);
            assert_eq!(hex(&public_key(&sk)), hex(&pk));
            assert_eq!(hex(&sign(&sk, &msg)), hex(&sig));
            assert!(verify(&pk, &msg, &sig));
        }
    }

    #[test]
    fn tampering_is_refused() {
        let (sk, pk, _, _) = VECTORS[2];
        let sk: [u8; 32] = arr(sk);
        let pk: [u8; 32] = arr(pk);
        let msg = b"OOS update package".to_vec();
        let sig = sign(&sk, &msg);
        assert!(verify(&pk, &msg, &sig));
        let mut m2 = msg.clone();
        m2[3] ^= 1;
        assert!(!verify(&pk, &m2, &sig));
        for i in [0, 31, 32, 63] {
            let mut s2 = sig;
            s2[i] ^= 0x10;
            assert!(!verify(&pk, &msg, &s2), "signature byte {} flipped", i);
        }
        // another key
        let other = public_key(&[7; 32]);
        assert!(!verify(&other, &msg, &sig));
        // S + L (malleability) refused
        let mut s = words::<4>(&sig[32..]);
        let mut c = 0u128;
        for i in 0..4 {
            let v = s[i] as u128 + L[i] as u128 + c;
            s[i] = v as u64;
            c = v >> 64;
        }
        let mut s3 = sig;
        for i in 0..4 {
            s3[32 + 8 * i..40 + 8 * i].copy_from_slice(&s[i].to_le_bytes());
        }
        assert!(!verify(&pk, &msg, &s3));
        // a key that is not a curve point
        let mut bad = pk;
        bad[0] ^= 1;
        let _ = verify(&bad, &msg, &sig); // must not panic
    }

    #[test]
    fn order_of_the_base_point_is_l() {
        use curve25519_dalek::traits::Identity;
        use curve25519_dalek::{constants::ED25519_BASEPOINT_POINT, edwards::EdwardsPoint};

        let mut p = EdwardsPoint::identity();
        let mut base = ED25519_BASEPOINT_POINT;
        for word in L {
            for bit in 0..64 {
                if word & (1 << bit) != 0 {
                    p += base;
                }
                base += base;
            }
        }
        assert_eq!(p, EdwardsPoint::identity());
    }

    #[test]
    fn noncanonical_scalars_and_small_order_points_are_refused() {
        let (_, pk, msg, sig) = VECTORS[1];
        let pk = arr(pk);
        let msg = unhex(msg).unwrap();
        let sig: [u8; 64] = arr(sig);
        for scalar in [L, [u64::MAX; 4]] {
            let mut bad = sig;
            for (chunk, word) in bad[32..].as_chunks_mut::<8>().0.iter_mut().zip(scalar) {
                chunk.copy_from_slice(&word.to_le_bytes());
            }
            assert!(!verify(&pk, &msg, &bad));
        }
        let mut identity = [0; 32];
        identity[0] = 1;
        let mut forged = [0; 64];
        forged[..32].copy_from_slice(&identity);
        assert!(!verify(&identity, &msg, &forged));
        assert!(!verify(&pk, &msg, &forged));
        assert!(!verify(&[0xff; 32], &msg, &sig));
    }
}
