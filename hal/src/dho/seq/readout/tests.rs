// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::dho::samples::test_support::noise;
use crate::dho::seq::{plan, test_support::writes, Segments, State, Write};

// Break it catches: an enable bit written with k = 0 (the read stalls), k beyond the 14-bit field (it wraps), a
// TX length that asks for samples beyond the record (the c2h read of k = 256 at 1 M timed out).
#[test]
fn compression_word_and_length_stay_inside_the_hardware() {
    assert_eq!(compress_word(0), 0x4000_0001);
    assert_eq!(compress_word(1), 0x4000_0001);
    assert_eq!(compress_word(4), 0xc000_0004);
    assert_eq!(compress_word(16383), 0xc000_3fff);
    assert_eq!(compress_word(16384), 0xc000_3fff);
    assert_eq!(compress_word(u32::MAX), 0xc000_3fff);
    for memsel in [1_000u32, 14_064, 62_504, 70_312, 100_000, 999_992, 1_000_000, 10_000_000, 125_000_000] {
        for k in [0u32, 1, 2, 3, 4, 15, 16, 25, 64, 128, 200, 256, 1000, 16383, 20000, u32::MAX] {
            let p = hz::Params {
                ratio: 1,
                rate: 1_250_000_000,
                memsel,
                dot_fs: 800_000,
                seg: 0,
                max_frames: 1,
                pre: memsel / 2,
                post: memsel / 2,
            };
            let s = Settings { compress: k, ..Settings::default() };
            let (kk, n) = (compress_k(&s, &p), compressed_len(&s, &p));
            assert!((1..=COMPRESS_K_MAX).contains(&kk) && n % 8 == 0 && n >= 8, "{} {}: k {} n {}", memsel, k, kk, n);
            assert!(n as u64 * kk as u64 <= memsel as u64, "{} at k {}: {} points x {}", memsel, k, n, kk);
            // at most 8 points (4 pairs) short of the record
            assert!(memsel as u64 - (n as u64 * kk as u64) < 8 * kk as u64, "{} at k {}", memsel, k);
        }
    }
    // the unit's cases: k 256 at 1 M asked 3912 points, now 3904; k 128 7816 -> 7808, k 4 250 000 unchanged
    let at = |k: u32| {
        compressed_len(
            &Settings { compress: k, ..Settings::default() },
            &hz::Params {
                ratio: 1,
                rate: 1_250_000_000,
                memsel: 1_000_000,
                dot_fs: 800_000,
                seg: 0,
                max_frames: 1,
                pre: 500_000,
                post: 500_000,
            },
        )
    };
    assert_eq!((at(256), at(128), at(4)), (3904, 7808, 250_000));
}

#[test]
fn frames_padded_to_8_bytes() {
    assert_eq!(
        (frame_stride(2500), frame_stride(1250), frame_stride(5000), frame_stride(500)),
        (2504, 1256, 5000, 504)
    );
    assert_eq!(read_len(4, 2500), 10016);
    // three frames of 5 bytes at a stride of 8
    let mut b: Vec<u8> = vec![1, 2, 3, 4, 5, 0, 0, 0, 6, 7, 8, 9, 10, 0, 0, 0, 11, 12, 13, 14, 15, 0, 0, 0];
    unpad(&mut b, 5);
    assert_eq!(b, (1..=15).collect::<Vec<u8>>());
}

#[test]
fn four_channel_window_like_phase05_step_d() {
    // step D (2026-10-09, word-identical): 4 channels, 4000 points from point 0 -> 0x1100 = 0x17e8, 0x1108 = 0,
    // 0x11f0 = 0xfa0, one read of 32000 bytes
    let s = Settings { mask: 0xf, tdiv: 2e-6, window: Some((4000, -1_000_000)), ..Settings::default() };
    let mut state = State::handover(0xf, None);
    let (st, p) = plan(&mut state, &s, None, None).unwrap();
    assert_eq!(p.ratio, 4);
    let w = writes(&st);
    assert!(w.contains(&(0x1100, 0x17e8)) && w.contains(&(0x1108, 0)) && w.contains(&(0x11f0, 0xfa0)));
    assert!(matches!(st.last(), Some(Step::Read { frames: 1, frame: 32000 })));
}

// Break it catches: an unpad on the reused DMA buffer that differs from the vector one.
#[test]
fn unpad_in_place_equals_unpad() {
    for frame in [5usize, 2500, 2504, 1250, 16] {
        let b = noise(7 * frame_stride(frame) + 3, frame as u32);
        let mut v = b.clone();
        unpad(&mut v, frame);
        let mut s = b.clone();
        let n = unpad_in(&mut s, frame);
        assert_eq!(&s[..n], &v[..], "frame {}", frame);
    }
}

// Break it catches: a segment read that plays from frame 0, a stamp play with the 8-point TX length the stock
// export never uses, a stamp play without the header.
#[test]
fn segment_reads_play_from_their_frame() {
    let s = Settings {
        mask: 3,
        tdiv: 1e-6,
        depth: 1000,
        window: None,
        segments: Some(Segments { n: 500, interval: 0.0 }),
        header: true,
        ..Settings::default()
    };
    let mut a = State::handover(3, None);
    let p = a.configure(&s, None, None).unwrap().1;
    let mut b = State::handover(3, None);
    b.configure(&s, None, None).unwrap();
    let at = writes(&a.read_steps_at(&s, &p, 200, 40));
    let zero = writes(&b.read_steps(&s, &p, 40));
    let diff: Vec<(Write, Write)> = at.iter().zip(&zero).filter(|(x, y)| x != y).map(|(x, y)| (*x, *y)).collect();
    assert_eq!(diff, [((0x4010, 200), (0x4010, 0)), ((0x4014, 200), (0x4014, 0)), ((0x4018, 239), (0x4018, 39))]);
    assert!(at.contains(&(0x1010, a.sh.s1010)) && a.sh.s1010 & 1 << 31 == 0, "header on");
    let st = a.segment_stamp_steps(&s, &p, 7);
    let w = writes(&st);
    // 128 words of two channels: TX length 64 per channel, one 16 + 256 byte read, then the latch pulse
    assert!(w.contains(&(0x11f0, 64)) && w.contains(&(0x4010, 7)) && w.contains(&(0x4018, 7)));
    assert!(w.iter().any(|x| x.0 == 0x1010 && x.1 & 1 << 31 == 0));
    assert_eq!(&w[w.len() - 3..], &[(0x40b8, 0), (0x40b8, 1), (0x40b8, 0)]);
    assert!(st.iter().any(|x| matches!(x, Step::Read { frames: 1, frame: 272 })));
    assert!(w.iter().position(|x| x.0 == 0x40b8).unwrap() > w.iter().position(|x| *x == (0x4004, a.sh.s4004)).unwrap());
}
