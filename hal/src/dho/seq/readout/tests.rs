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
            let s = Settings { compress: k, window: None, ..Settings::default() };
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
            &Settings { compress: k, window: None, ..Settings::default() },
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
fn compressed_batch_registers_use_the_played_window() {
    for (at, start) in [(-32_312, 17_688), (i64::MIN, 0), (i64::MAX, 35_368)] {
        let s = Settings {
            tdiv: 10e-6,
            depth: 100_000,
            frames: 30,
            compress: 15,
            window: Some((64_632, at)),
            header: true,
            ..Settings::default()
        };
        let (steps, p) = plan(&mut State::handover(1, None), &s, None, None).unwrap();
        assert_eq!((p.memsel, p.pre), (100_000, 50_000));
        assert_eq!(window(&s, &p), (64_632, start));
        assert_eq!((compress_k(&s, &p), compressed_len(&s, &p)), (15, 4304));
        let writes = writes(&steps);
        assert!(writes.contains(&(0x1100, 64_632 + 0x248)));
        assert!(writes.contains(&(0x1108, start)));
        assert!(writes.contains(&(0x11f0, 4304)));
        assert!(writes.contains(&(0x1170, 0xc000_000f)));
        assert!(writes.windows(3).any(|w| w == [(0x4010, 0), (0x4014, 0), (0x4018, 29)]));
        let frame = frame_bytes(&s, &p);
        assert_eq!((frame, frame_stride(frame)), (8624, 8624));
        assert!(matches!(steps.last(), Some(Step::Read { frames: 30, frame: 8624 })));
    }
}

#[test]
fn invalid_compression_never_produces_play_steps() {
    let mut st = State::handover(1, None);
    let mut s =
        Settings { tdiv: 100e-9, depth: 1000, window: Some((24, -500)), compress: u32::MAX, ..Settings::default() };
    let (_, p) = st.configure(&s, None, None).unwrap();
    assert_eq!((compress_k(&s, &p), compressed_len(&s, &p)), (3, 8));
    for (window, error) in [
        ((24, -499), ReadoutError::UnalignedWindow { start: 1, points: 24 }),
        ((25, -500), ReadoutError::UnalignedWindow { start: 0, points: 25 }),
        ((0, -500), ReadoutError::InsufficientPoints { points: 0, factor: 1, len: 8 }),
        ((4, -500), ReadoutError::InsufficientPoints { points: 4, factor: 1, len: 8 }),
    ] {
        s.window = Some(window);
        assert_eq!(validate_readout(&s, &p), Err(error));
        assert_eq!(plan(&mut st, &s, None, None).unwrap_err(), error.to_string());
        assert_eq!(st.acquire(&s, &p).unwrap_err(), error);
        assert_eq!(st.read_steps(&s, &p, 2).unwrap_err(), error);
        assert_eq!(st.read_steps_at(&s, &p, 2, 3).unwrap_err(), error);
        assert_eq!(st.reread_steps(&s, &p, 3).unwrap_err(), error);
    }
}

#[test]
fn logic_analyser_records_keep_compressed_playback() {
    // Deep records with the logic analyser on are read compressed: the planner has no window for them.
    let mut s = Settings { la: Some(la::Set::default()), compress: 1, window: None, ..Settings::default() };
    let mut st = State::handover(1, None);
    let (_, p) = plan(&mut st, &s, None, None).unwrap();
    s.compress = 4;
    assert_eq!(validate_readout(&s, &p), Ok(()));
    assert!(st.acquire(&s, &p).is_ok());
    assert!(st.reread_steps(&s, &p, 0).is_ok());
}

#[test]
fn ring_simulation_plays_windowed_pairs_per_lane() {
    use crate::dho::bus::DmaBuf;
    use crate::dho::seg::{header_stamp, sim};
    use crate::dho::seq::execute_bus;

    let s = Settings {
        mask: 3,
        tdiv: 100e-9,
        depth: 1000,
        header: true,
        compress: 3,
        window: Some((24, -380)),
        ..Settings::default()
    };
    let mut st = State::handover(3, None);
    let (config, p) = st.configure(&s, None, None).unwrap();
    let (_, mut bus, mut stream) = sim::engine(sim::Engine::new(10_000_000, 8_000_000, 1, p.ratio));
    let mut dma = DmaBuf::new();
    execute_bus(&config, &mut bus, &mut stream, &mut dma, |_| {}).unwrap();
    let steps = st.read_steps_at(&s, &p, 2, 3).unwrap();
    execute_bus(&steps, &mut bus, &mut stream, &mut dma, |_| {}).unwrap();

    assert_eq!(dma.data().len(), 3 * 48);
    for (i, data) in dma.data().as_chunks::<48>().0.iter().enumerate() {
        assert_eq!(header_stamp(data), Some((18_000_000 + i as u64 * 8_000_000) / 800_000));
        let base = (i + 2) as u8;
        // The second block wraps the low byte, so its maximum comes before its minimum.
        let expected = [240, 241, 250, 251, 254, 255, 0, 1, 8, 9, 18, 19, 20, 21, 30, 31];
        assert!(data[16..].as_chunks::<2>().0.iter().zip(expected).all(|(word, sample)| *word == [sample, base]));
    }
}

#[test]
fn ring_simulation_pads_each_frame_without_leaving_bytes_for_the_next_play() {
    use crate::dho::bus::DmaBuf;
    use crate::dho::seg::sim;
    use crate::dho::seq::execute_bus;

    let s = Settings { tdiv: 100e-9, window: Some((1250, i64::MIN)), ..Settings::default() };
    let mut st = State::handover(1, None);
    let (_, p) = st.configure(&s, None, None).unwrap();
    let frame = frame_bytes(&s, &p);
    assert_eq!((frame, frame_stride(frame)), (2500, 2504));
    let (_, mut bus, mut stream) = sim::engine(sim::Engine::new(0, 800_000, 1, p.ratio));
    let mut dma = DmaBuf::new();
    for (first, count) in [(2, 3), (8, 1)] {
        let steps = st.read_steps_at(&s, &p, first, count).unwrap();
        execute_bus(&steps, &mut bus, &mut stream, &mut dma, |_| {}).unwrap();
        assert_eq!(dma.data().len(), count as usize * frame);
        for (index, data) in dma.data().chunks_exact(frame).enumerate() {
            let index = first + index as u32;
            assert!(data
                .as_chunks::<2>()
                .0
                .iter()
                .enumerate()
                .all(|(i, word)| { u16::from_le_bytes(*word) == sim::Engine::word(index, i) }));
        }
    }
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
    let at = writes(&a.read_steps_at(&s, &p, 200, 40).unwrap());
    let zero = writes(&b.read_steps(&s, &p, 40).unwrap());
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
