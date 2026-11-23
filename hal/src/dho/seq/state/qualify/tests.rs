// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::dho::seq::{test_support::writes, HwMask, HwZone};
use crate::dho::Write;

#[test]
fn hardware_mask_starts_reloads_clears_and_stops() {
    let only = |st: &mut State, s: &Settings| -> Vec<Write> {
        writes(&st.configure(s, None, None).unwrap().0)
            .into_iter()
            .filter(|w| matches!(w.0, 0x1184 | 0x1190 | 0x1194) || w.0 == 0x4020)
            .collect()
    };
    let band = |h: f32| HwMask {
        ch: 0,
        lo: vec![-h; mask::COLUMNS],
        hi: vec![h; mask::COLUMNS],
        stop_on_fail: false,
        clear: 0,
        map: mask::Map { skew: 0.0, ..mask::Map::default() },
    };
    let mut st = State::handover(1, None);
    let base = Settings { window: None, ..Settings::default() };
    let on = Settings { hwmask: Some(band(1.0)), ..base.clone() };
    let w = only(&mut st, &on);
    // off, 1000 columns + write enable dropped, compression for 2.5 Mpts, slot 0 with the counter clear, on; then
    // the auto-trigger block writes 0x4020 from the same shadow
    assert_eq!(w.len(), 1 + 1001 + 1 + 2 + 1 + 1, "{:x?}", &w[..4]);
    assert_eq!(w[0], (0x4020, 0x2001));
    assert_eq!(w[1], (0x1194, 0x1c00_0000 | 158 << 8 | 98));
    assert_eq!(w[1001], (0x1194, 0));
    assert_eq!(w[1002..1006], [(0x1184, 0xc000_03ff), (0x1190, 0x6010_0000), (0x1190, 0x0010_0000), (0x4020, 0xe001)]);
    assert_eq!(*w.last().unwrap(), (0x4020, 0xe001), "auto trigger keeps the mask bits");
    assert_eq!(st.mask_ctrl(), Some(0x0010_0000));
    // unchanged: nothing but the auto-trigger write
    assert_eq!(only(&mut st, &on), vec![(0x4020, 0xe001)]);
    // a tighter band: the table again, the counters keep counting
    let w = only(&mut st, &Settings { hwmask: Some(band(0.5)), ..base.clone() });
    assert_eq!(w.len(), 1 + 1001 + 1 + 1 + 1 + 1);
    assert_eq!(w[1003], (0x1190, 0x0010_0000));
    // a clear request: no table, the counters cleared
    let w = only(&mut st, &Settings { hwmask: Some(HwMask { clear: 1, ..band(0.5) }), ..base.clone() });
    assert_eq!(
        w,
        vec![
            (0x4020, 0x6001),
            (0x1184, 0xc000_03ff),
            (0x1190, 0x6010_0000),
            (0x1190, 0x0010_0000),
            (0x4020, 0xe001),
            (0x4020, 0xe001)
        ]
    );
    // stop on fail sets bit 12
    let w =
        only(&mut st, &Settings { hwmask: Some(HwMask { clear: 1, stop_on_fail: true, ..band(0.5) }), ..base.clone() });
    assert_eq!(*w.last().unwrap(), (0x4020, 0xf001));
    // off: enable and stop cleared, compression back, slot dropped
    let w = only(&mut st, &base);
    assert_eq!(w, vec![(0x4020, 0x6001), (0x1184, 0x4000_0001), (0x1190, 0), (0x4020, 0x6001)]);
    assert_eq!(st.mask_ctrl(), None);
    assert_eq!(only(&mut st, &base), vec![(0x4020, 0x6001)]);
    // a source that is not acquired leaves the test off
    let w = only(&mut st, &Settings { hwmask: Some(HwMask { ch: 1, ..band(1.0) }), ..base.clone() });
    assert_eq!(w, vec![(0x4020, 0x6001)]);
    // CH2 of CH1+CH2: slot 1 (stream slot)
    let two = Settings { mask: 0b11, hwmask: Some(HwMask { ch: 1, ..band(1.0) }), ..base.clone() };
    let mut st = State::handover(0b11, None);
    assert!(only(&mut st, &two).contains(&(0x1190, 0x0020_0000)));
}

#[test]
fn hardware_zone_starts_hands_over_to_the_mask_and_stops() {
    let only = |st: &mut State, s: &Settings| -> Vec<Write> {
        writes(&st.configure(s, None, None).unwrap().0)
            .into_iter()
            .filter(|w| matches!(w.0, 0x1184 | 0x1190 | 0x1194 | 0x11d0 | 0x11d8 | 0x11dc | 0x4020))
            .collect()
    };
    let bx = |inside| zone::Zone { poly: vec![(-1e-4, 0.5), (1e-4, 1.5)], inside };
    let z1 = HwZone { ch: 0, zones: vec![bx(true)], clear: 0, map: mask::Map::default() };
    let base = Settings { window: None, ..Settings::default() };
    let on = Settings { hwzone: Some(z1.clone()), ..base.clone() };
    let mut st = State::handover(1, None);
    let w = only(&mut st, &on);
    // off, table 1 (1000 + 1), control, compression, slot with clear, armed; then the auto-trigger write
    assert_eq!(w.len(), 1 + 1001 + 1 + 1 + 2 + 1 + 1);
    assert_eq!(w[0], (0x4020, 0x2001));
    assert!(w[1..1001].iter().all(|x| x.0 == 0x11d8));
    assert_eq!(
        w[1002..1008],
        [
            (0x11d0, 0x0500_0000),
            (0x1184, 0xc000_03ff),
            (0x1190, 0x6010_0000),
            (0x1190, 0x0010_0000),
            (0x4020, 0x2003),
            (0x4020, 0x2003)
        ]
    );
    assert_eq!(st.zone_ctrl(), Some(0x0010_0000));
    assert_eq!(only(&mut st, &on), vec![(0x4020, 0x2003)]);
    // a second zone that must not be touched: its table, the new control word, no clear
    let two = Settings { hwzone: Some(HwZone { zones: vec![bx(true), bx(false)], ..z1.clone() }), ..base.clone() };
    let w = only(&mut st, &two);
    assert!(
        w.iter().filter(|x| x.0 == 0x11dc).count() == 1001
            && w.contains(&(0x11d0, 0x0700_0000))
            && w.contains(&(0x1190, 0x0010_0000))
            && !w.contains(&(0x1190, 0x6010_0000))
    );
    // the mask test wins: the zone logic off, the mask keeps the slot it sets
    let mut m = Settings {
        hwmask: Some(HwMask {
            ch: 0,
            lo: vec![-1.0; mask::COLUMNS],
            hi: vec![1.0; mask::COLUMNS],
            stop_on_fail: false,
            clear: 0,
            map: mask::Map::default(),
        }),
        ..two.clone()
    };
    let w = only(&mut st, &m);
    assert_eq!(&w[..2], [(0x4020, 0x2001), (0x11d0, 0)]);
    assert_eq!(st.zone_ctrl(), None);
    assert!(st.mask_ctrl().is_some() && *w.last().unwrap() == (0x4020, 0xe001));
    // back to the zone: the mask goes off without taking the zone's slot and strobe
    m.hwmask = None;
    let w = only(&mut st, &m);
    assert_eq!(st.zone_ctrl(), Some(0x0010_0000));
    assert!(!w.contains(&(0x1184, 0x4000_0001)), "{:x?}", w);
    assert_eq!(*w.last().unwrap(), (0x4020, 0x6003));
    // off
    let w = only(&mut st, &base);
    assert_eq!(w, vec![(0x4020, 0x6001), (0x11d0, 0), (0x1184, 0x4000_0001), (0x1190, 0), (0x4020, 0x6001)]);
}
