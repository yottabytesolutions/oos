// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::dho::bus::{DmaBuf, Recorder};
use std::io::Cursor;

fn stock(mask: u32, depth: i128) -> Settings {
    Settings { mask, depth, tdiv: 1e-6, window: None, playback: Playback::StockInferred, ..Default::default() }
}

#[test]
fn stock_play_agrees_with_the_register_sequence_and_exact_dma_stride() {
    for (mask, depth, start, points, len, increment) in [
        (1, 14_064, 782, 12_500, 13_000, 0xf627_6275),
        (3, 7_032, 391, 6_250, 7_000, 0xe492_4923),
        (15, 3_520, 197, 3_125, 4_000, 0xc7ff_ffff),
    ] {
        for header in [false, true] {
            let s = Settings { header, ..stock(mask, depth) };
            let mut state = State::handover(mask, None);
            let (configured, p) = state.configure(&s, None, None).unwrap();
            let mut raw = State::handover(mask, None);
            let (raw_config, raw_p) = raw.configure(&raw_settings(&s), None, None).unwrap();
            assert_eq!((format!("{configured:?}"), p), (format!("{raw_config:?}"), raw_p));
            let steps = state.read_steps_at(&s, &p, 1, 3);
            let fb = (2 * len * p.ratio) as usize + 16 * usize::from(header);
            assert!(matches!(steps.last(), Some(Step::Read { frames: 3, frame }) if *frame == fb));
            assert_eq!((frame_len(&s, &p), frame_bytes(&s, &p), frame_stride(fb)), (len, fb, fb));

            let mut bar = Recorder::default();
            let mut dma = DmaBuf::new();
            let expected = [0x33, 0x55, 0x77].into_iter().flat_map(|v| vec![v; fb]).collect::<Vec<_>>();
            let mut stream = Cursor::new([expected.as_slice(), &[0xff]].concat());
            execute_bus(&steps, &mut bar, &mut stream, &mut dma, |_| {}).unwrap();
            assert_eq!(dma.data(), expected);
            assert_eq!(stream.position() as usize, expected.len());
            for (reg, val) in [
                (0x1100, points + (0x48 | (p.ratio << 9)) + 1),
                (0x1108, start),
                (0x1130, 0x0001_0101),
                (0x1154, 2),
                (0x1158, 0),
                (0x115c, increment),
                (0x1170, 0x4000_0001),
                (0x11f0, len),
                (0x11fc, 1000),
                (0x4010, 1),
                (0x4018, 3),
            ] {
                assert_eq!(bar.writes.iter().rev().find(|write| write.0 == reg), Some(&(reg, val)));
            }
        }
    }
}

#[test]
fn unverified_or_incompatible_play_uses_the_exact_raw_step_list() {
    let s = stock(1, 14_064);
    for want in [
        Settings { playback: Playback::Stock, ..s.clone() },
        Settings { tdiv: 5e-9, ..s.clone() },
        Settings { tdiv: 800e-9, ..s.clone() }, // 10 input samples per column, so no interpolation.
        Settings { tdiv: 2e-6, ..s.clone() },
        Settings { mask: 7, ..s.clone() },
        Settings { mode: acq::Mode::Peak, ..s.clone() },
        Settings { window: Some((4000, -2000)), ..s.clone() },
        Settings { depth: 12_500, ..s.clone() }, // Last phase needs the sample just after this record.
        Settings { delay: 1e-9, ..s.clone() },
        Settings { compress: 4, ..s },
    ] {
        let mut state = State::handover(want.mask, None);
        let mut raw = State::handover(want.mask, None);
        let raw_want = Settings { playback: Playback::Raw, ..want.clone() };
        let (steps, p) = plan(&mut state, &want, None, None).unwrap();
        let (raw_steps, raw_p) = plan(&mut raw, &raw_want, None, None).unwrap();
        assert!(stock_play(&want, &p).is_none(), "{want:?}");
        assert_eq!((format!("{steps:?}"), p), (format!("{raw_steps:?}"), raw_p));
    }
}

#[test]
fn the_raw_reread_disables_linear_play() {
    let s = stock(1, 14_064);
    let mut state = State::handover(1, None);
    let (_, p) = state.configure(&s, None, None).unwrap();
    assert!(stock_play(&s, &p).is_some());
    let raw = raw_settings(&s);
    let steps = state.reread_steps(&raw, &p, 6);
    assert_eq!(frame_len(&raw, &p), 14_064);
    assert!(steps.iter().any(|step| matches!(step, Step::Wr((0x4010, 6)))));
    assert!(steps
        .iter()
        .rev()
        .find_map(|step| match step {
            Step::Wr((0x1154, value)) => Some(*value == 3),
            _ => None,
        })
        .unwrap());
    assert!(matches!(steps.last(), Some(Step::Read { frames: 1, frame: 28_128 })));
}
