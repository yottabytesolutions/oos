// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

#[test]
fn decode_and_align_a_square() {
    // a 2-channel record: CH1 square 0/2.5 V (1 kHz), CH2 constant; 100 k points at 625 MSa/s -> fake 1 MSa/s here
    let m = Model::default();
    let p = hz::Params {
        ratio: 2,
        rate: 1_000_000,
        memsel: 4000,
        dot_fs: 1_000_000_000,
        seg: 0,
        max_frames: 0,
        pre: 2000,
        post: 2000,
    };
    let s =
        Settings { mask: 3, ch: [Chan { scale: 1.0, bw: 0, ..Settings::default().ch[0] }; 4], ..Settings::default() };
    let mut bytes = Vec::new();
    for i in 0..4000usize {
        // rising edge of CH1 at sample 2003 (3 samples after the hardware trigger point)
        let v1 = if (i + 1000 - 3) % 1000 < 500 { 2.5 } else { 0.0 };
        for v in [v1, 0.5] {
            let w = (32768.0 + v * 7500.0) as u16;
            bytes.extend(w.to_le_bytes());
        }
    }
    let f = decode(&bytes, &s, &p, 1);
    assert_eq!(f.ch[0].len(), 4000);
    assert!((f.ch[1][10] - 0.5).abs() < 1e-3);
    let window = 1e-3;
    let r = to_record(&Arc::new(f), &m, 1000, window, -window / 2.0);
    assert!(r.triggered);
    // the software edge (level 1.5 V) sits at the centre of the screen window
    assert!(r.ch[0][495] < 0.5 && r.ch[0][505] > 2.0, "{} {}", r.ch[0][495], r.ch[0][505]);
}

#[test]
fn raw_pair_layout_skips_header_and_stride_padding() {
    use crate::fastdraw::Samples;
    use oos_hal::dho::seq;
    for (mask, lanes) in [
        (8, [None, None, None, Some(0)]),
        (10, [None, Some(0), None, Some(1)]),
        (13, [Some(0), None, Some(2), Some(3)]),
        (15, [Some(0), Some(1), Some(2), Some(3)]),
    ] {
        let s = Settings {
            mask,
            header: true,
            playback: seq::Playback::Last,
            compress: 3,
            window: Some((19, -29)),
            ..Settings::default()
        };
        let mut p = hz::prep(mask, 1_000_000_000, 100, 0);
        p.pre = 50;
        let end = seq::frame_bytes(&s, &p);
        let mut bytes = vec![0xa5; seq::frame_stride(end)];
        for (i, word) in bytes[16..end].as_chunks_mut::<2>().0.iter_mut().enumerate() {
            *word = (30000 + i as u16).to_le_bytes();
        }

        let expected = decode(&bytes[..end], &s, &p, 7);
        let mut frame = expected.clone();
        let layout = codes_into(&bytes, &s, &p, 7, &mut frame).unwrap();
        assert_eq!(layout.word, lanes);
        assert_eq!(layout.stride, p.ratio as usize);
        assert!(frame.envelope && frame.ch.iter().all(Vec::is_empty));
        assert_eq!(
            (frame.seq, frame.pre, frame.dt, frame.trigger_anchor.at_adc_rate()),
            (7, expected.pre, expected.dt, expected.trigger_anchor.at_adc_rate())
        );
        for c in 0..4 {
            let samples = layout.channel(&bytes, c);
            assert_eq!(samples.n, expected.ch[c].len());
            for (i, value) in expected.ch[c].iter().enumerate() {
                assert_eq!(samples.at(i).to_bits(), value.to_bits());
            }
        }
        assert!(codes_into(&bytes[..16], &s, &p, 7, &mut frame).is_none());
    }
}
