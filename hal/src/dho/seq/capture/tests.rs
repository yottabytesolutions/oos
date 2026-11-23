// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::dho::acq;
use crate::dho::seq::{frame_stride, plan, read_len, State, Step};
use std::fmt::Write as _;

fn full_steps() -> String {
    let mut out = String::new();
    for (mask, depth) in [(1, 14_064), (3, 7032), (15, 3520)] {
        let mut state = State::handover(mask, None);
        let s = Settings { mask, depth, tdiv: 1e-6, frames: 4, ..Settings::default() };
        let (steps, _) = plan(&mut state, &s, None, None).unwrap();
        writeln!(out, "mask {mask:#x}").unwrap();
        for step in steps {
            writeln!(out, "{step:?}").unwrap();
        }
    }
    out
}

#[test]
fn full_capture_matches_the_baseline_steps() {
    assert_eq!(full_steps(), include_str!("../full-capture-steps.txt"));
}

#[test]
fn stock_decimation_uses_documented_fixed_depth_tuples() {
    for (mask, depth, factor, rate) in [
        (1, 14_064, 2, 625_000_000),
        (1, 14_064, 4, 312_500_000),
        (1, 14_064, 8, 156_250_000),
        (3, 19_000, 2, 312_500_000),
        (3, 19_000, 4, 156_250_000),
        (15, 19_000, 2, 156_250_000),
    ] {
        let mut s = Settings { mask, depth, tdiv: 1e-6, window: None, ..Settings::default() };
        let p = s.reduce_capture(false, factor, false).unwrap();
        assert_eq!((p.rate, p.memsel), (rate, 10_000));
        assert_read_geometry(&s, p, factor, 0);
    }
}

#[test]
fn inferred_geometry_is_aligned_for_all_exact_factors_and_masks() {
    for (mask, max_rate, last) in [(1, 1_250_000_000, 16), (3, 625_000_000, 8), (15, 312_500_000, 4)] {
        for factor in [2, 4, 8, 16, 32, 64] {
            for peak in [false, true] {
                let mut s =
                    Settings { mask, depth: 14_064, tdiv: 1e-6, header: true, window: None, ..Settings::default() };
                let actual = s.reduce_capture(peak, factor, true);
                if factor > last {
                    assert!(actual.is_none());
                    assert_eq!(s.mode, acq::Mode::Normal);
                    continue;
                }
                let p = actual.unwrap();
                assert_eq!(p.rate, max_rate / u64::from(factor));
                assert_eq!(p.memsel, 14_064 / factor / 8 * 8);
                let mode = u32::from(peak) * 2 + u32::from(factor > 8) * 4;
                assert_read_geometry(&s, p, factor, mode);
            }
        }
    }
}

fn assert_read_geometry(s: &Settings, planned: hz::Params, factor: u32, mode: u32) {
    let mut state = State::handover(s.mask, None);
    let (steps, p) = state.configure(s, None, None).unwrap();
    assert_eq!(p, planned);
    let last = |reg| {
        steps.iter().rev().find_map(|step| match *step {
            Step::Wr((address, value)) if address == reg => Some(value),
            _ => None,
        })
    };
    assert_eq!(last(0x10d0), Some(0x8000 | factor));
    assert_eq!((last(0x10d8), last(0x10d4)), (Some(1), Some(0)));
    assert_eq!(last(0x1004).unwrap() & 7, mode);
    assert_eq!(last(0x10e4), Some(p.memsel));
    assert_eq!((last(0x4050), last(0x4054)), (Some(p.memsel / 2), Some(p.memsel / 2)));
    assert_eq!(last(0x10e0), Some(hz::mem_size(i128::from(p.memsel * p.ratio)) as u32));
    let read = state.read_steps(s, &p, 3).unwrap();
    assert!(read.iter().any(|step| matches!(step, Step::Wr((0x11f0, n)) if *n == p.memsel)));
    assert!(read.iter().any(|step| matches!(step, Step::Wr((0x1170, 0x4000_0001)))));
    let frame = (2 * p.memsel * p.ratio) as usize + if s.header { 16 } else { 0 };
    assert!(matches!(read.last(), Some(Step::Read { frames: 3, frame: actual }) if *actual == frame));
    assert_eq!(frame_stride(frame), frame);
    assert_eq!(read_len(3, frame), 3 * frame);
}

#[test]
fn inferred_capture_needs_opt_in_and_ordinary_full_rate_input() {
    let full = Settings { depth: 14_064, tdiv: 1e-6, window: None, ..Settings::default() };
    for (peak, factor) in [(true, 2), (false, 16), (false, 3)] {
        let mut s = full.clone();
        assert!(s.reduce_capture(peak, factor, false).is_none());
        assert_eq!(s, full);
    }
    for mut s in [
        Settings { depth: 1_000, ..full.clone() },
        Settings { delay: 1e-9, ..full.clone() },
        Settings { mask: 7, ..full.clone() },
        Settings { compress: 2, ..full.clone() },
        Settings { mode: acq::Mode::Average { count: 16 }, ..full.clone() },
    ] {
        let before = s.clone();
        assert!(s.reduce_capture(false, 2, true).is_none());
        assert_eq!(s, before);
    }
    let mut s = full;
    s.reduce_capture(false, 2, true).unwrap();
    s.compress = 2;
    assert!(State::handover(1, None).configure(&s, None, None).is_err());
}
