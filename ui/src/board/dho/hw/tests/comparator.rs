// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

#[test]
fn mask_map_from_conf_lines() {
    use oos_hal::dho::mask::{Domain, Layout, Map};
    assert_eq!(mask_map_from(""), Map::default());
    let m =
        mask_map_from("mask.hw = on\nmask.map.domain = pregain\nmask.map.cols = stock # sweep\nmask.map.shift = x\n");
    assert_eq!((m.domain, m.layout, m.shift), (Domain::PreGain, Layout::Stock, 8));
}

#[test]
fn zone_batches_hold_zone_batch_frames() {
    assert_eq!(
        (zone_batch_from(""), zone_batch_from("zone.batch = 8\n"), zone_batch_from("zone.batch = 0\n")),
        (1, 8, 1)
    );
    let m = Model {
        tscale: tscales().iter().position(|&t| (t as f64 - 1e-6).abs() < 1e-12).unwrap(),
        ..Default::default()
    };
    let s = settings(&m, false, &Default::default());
    let p = hz::prep(s.mask, oos_hal::dho::seq::tdiv_fs(s.tdiv), s.depth, 0);
    assert!(batch_plan(&s, &p, 1).frames > 1);
    let z = Settings {
        hwzone: Some(oos_hal::dho::seq::HwZone {
            ch: 0,
            zones: vec![],
            clear: 0,
            map: oos_hal::dho::mask::Map::default(),
        }),
        ..s
    };
    assert_eq!(batch_plan(&z, &p, 8).frames, 8);
}

/// Unit round 5: at 100 us/div with 10k points the record is 1.6 screens long, and a table spread over the record
/// failed every frame of a clean 1 kHz square. Mapped by time, the clean signal passes and a 10 us shift fails;
/// spread over the record (the old mapping) the clean signal fails.
#[test]
fn mask_table_follows_the_screen_when_the_record_is_longer() {
    use oos_hal::dho::mask;
    let (n, pre, rate) = (10_000usize, 5_000u32, 6_250_000u64);
    let dt = 1.0 / rate as f64;
    // 0 / 3 V, 1 kHz, rising edge at the trigger (`mask::SKEW` samples after `pre`, round 20); `shift` seconds later
    let wave = |shift: f64| -> Vec<f32> {
        (0..n)
            .map(|k| (k as f64 - pre as f64 - mask::SKEW) * dt - shift)
            .map(|t| if (t * 1e3).rem_euclid(1.0) < 0.5 { 3.0 } else { 0.0 })
            .collect()
    };
    // the screen: -0.5..0.5 ms = samples 1875..8125; the mask from it
    let (s0, s1) = (1875, 8125);
    let m = oos_dsp::mask::build(&wave(0.0)[s0..s1], 20, 0.3);
    let (lo, hi) = m.columns(mask::COLUMNS);
    let fails = |t: &mask::Timing, v: &[f32]| -> bool {
        let map = mask::Map::default();
        let cols = mask::table_map(&lo, &hi, t, 1.0, -1.5, 1.0, &map);
        let per = map.layout(t.points, 1).2;
        v.iter().enumerate().any(|(k, &x)| {
            let c = (k as f64 / per) as usize;
            let code = map.code(x as f64, 1.0, -1.5, 1.0).floor() as i32;
            c < cols.len()
                && ((cols[c].lo_en && code < cols[c].lo as i32) || (cols[c].hi_en && code > cols[c].hi as i32))
        })
    };
    let by_time = mask::Timing { points: n as u32, pre, rate, mask: 1, screen: (-0.5e-3, 1e-3) };
    assert!(!fails(&by_time, &wave(0.0)), "clean signal");
    assert!(fails(&by_time, &wave(10e-6)), "10 us shift");
    assert!(fails(&by_time, &wave(-10e-6)), "-10 us shift");
    let over_record = mask::Timing { screen: (-(pre as f64) * dt, n as f64 * dt), ..by_time };
    assert!(fails(&over_record, &wave(0.0)), "the old mapping fails the clean signal");
}

#[test]
fn a_stalling_unit_is_dropped_for_its_settings() {
    let map = oos_hal::dho::mask::Map::default();
    let s = Settings::default();
    let k = unit_key(&s, map);
    let other = unit_key(&Settings { depth: 100_000, ..s.clone() }, map);
    let mut g = UnitGuard::default();
    let t0 = std::time::Instant::now();
    // a stall without the unit, or a good batch with it in between, does not count
    assert!(
        !g.batch(true, false, k, t0)
            && !g.batch(true, true, k, t0)
            && !g.batch(false, true, k, t0)
            && !g.batch(true, true, k, t0)
    );
    // after a stall one batch runs without the unit; it keeps the count
    assert!(g.blocks(&k, t0) && g.blocks(&other, t0));
    assert!(!g.batch(false, false, k, t0) && !g.blocks(&k, t0));
    // two in a row with the unit running: blocked for these settings only
    assert!(g.batch(true, true, k, t0));
    assert!(!g.batch(false, false, k, t0));
    assert!(g.blocks(&k, t0) && !g.blocks(&other, t0));
    // another mapping tries again
    let swept = unit_key(&s, oos_hal::dho::mask::Map { layout: oos_hal::dho::mask::Layout::Clocks, ..map });
    assert!(!g.blocks(&swept, t0));
    // the first block lapses once; a second block of the same settings stays
    let later = t0 + UNIT_RETRY;
    assert!(!g.blocks(&k, later));
    assert!(!g.batch(true, true, k, later) && !g.batch(false, false, k, later) && g.batch(true, true, k, later));
    assert!(!g.batch(false, false, k, later));
    assert!(g.blocks(&k, later + UNIT_RETRY * 10));
}

#[test]
fn a_trigger_wait_is_no_stall_and_not_logged() {
    let wait = "no acquisition (0x4080 bit30 not set; 0x4030 = 0x0f041722, readback 0, state 2)";
    assert!(is_stall(wait) && !is_stall("c2h read of 4096 bytes: timed out"));
    // (stalled, unit) -> logged
    assert!(!logs_acquire_error(true, false));
    assert!(logs_acquire_error(true, true) && logs_acquire_error(false, false) && logs_acquire_error(false, true));
    // a wait without the unit neither counts nor flushes
    let k = unit_key(&Settings::default(), oos_hal::dho::mask::Map::default());
    let mut g = UnitGuard::default();
    let t0 = std::time::Instant::now();
    for _ in 0..5 {
        assert!(!g.batch(is_stall(wait), false, k, t0));
    }
    assert!(!g.blocks(&k, t0) && g.stalls.is_none());
}

#[test]
fn stalls_across_a_settings_change_do_not_block() {
    // round 19: one stall with the old settings, one with the new ones
    let map = oos_hal::dho::mask::Map::default();
    let s = Settings::default();
    let (old, new) = (unit_key(&s, map), unit_key(&Settings { depth: 100_000, ..s.clone() }, map));
    let mut g = UnitGuard::default();
    let t0 = std::time::Instant::now();
    assert!(!g.batch(true, true, old, t0));
    // round 20: the new settings run the flush batch first, then the unit again
    assert!(g.blocks(&new, t0) && !g.batch(false, false, new, t0) && !g.blocks(&new, t0));
    assert!(!g.batch(true, true, new, t0) && !g.batch(false, false, new, t0));
    assert!(!g.blocks(&new, t0) && !g.blocks(&old, t0));
}

#[test]
fn zone_hw_from_conf() {
    assert!(!zone_hw_from(""));
    assert!(zone_hw_from("zone.hw = on\n"));
}

#[test]
fn mask_and_zone_merge() {
    use crate::board::{MaskWant, ZoneWant};
    let mw = MaskWant { ch: 0, lo: vec![-1.0; 1000], hi: vec![1.0; 1000], stop: false, clear: 0 };
    let zw = ZoneWant {
        ch: 1,
        zones: vec![(vec![(0.0, 0.0), (1e-6, 1.0)], true), (vec![(0.0, 2.0), (1e-6, 3.0)], false), (vec![], true)],
        clear: 4,
    };
    let pf = auxout::AuxOut::PassFail(auxout::PassFail::DEFAULT);
    let mut s = Settings { aux: pf, ..Settings::default() };
    hw_merge(&mut s, None, Some(&zw), oos_hal::dho::mask::Map::default());
    let z = s.hwzone.as_ref().unwrap();
    assert_eq!((z.ch, z.zones.len(), z.zones[1].inside, z.clear), (1, 2, false, 4));
    assert_eq!(
        s.aux,
        auxout::AuxOut::PassFail(auxout::PassFail { source: auxout::Source::Zone, ..auxout::PassFail::DEFAULT })
    );
    // the mask test wins and keeps the mask source
    let mut s = Settings { aux: pf, ..Settings::default() };
    hw_merge(&mut s, Some(&mw), Some(&zw), oos_hal::dho::mask::Map::default());
    assert!(s.hwmask.is_some() && s.hwzone.is_none() && s.aux == pf);
    let mut s = Settings::default();
    hw_merge(&mut s, None, None, oos_hal::dho::mask::Map::default());
    assert!(s.hwmask.is_none() && s.hwzone.is_none() && s.aux == auxout::AuxOut::TriggerOut);
}

#[test]
fn mask_hw_from_conf() {
    assert!(!mask_hw_from(""));
    assert!(mask_hw_from("mask.hw = on\n"));
    assert!(!mask_hw_from("mask.hw = 1\n"));
}

/// The software mask in the hardware table, and the comparator modelled on the read-out words (the assumed sample
/// scale, `mask::code`): what the software passes the table passes, a runt fails both.
#[test]
fn hardware_table_agrees_with_the_software_mask() {
    use oos_hal::dho::mask;
    let (scale, offset) = (1.0, -1.5);
    let n = 20_000;
    // 5 periods of a 0..3 V square with a little noise, and a runt in period 3
    let wave = |runt: bool, seed: u32| -> Vec<f32> {
        (0..n)
            .map(|k| {
                let ph = k % 4000;
                let hi = if runt && k / 4000 == 3 { 1.2 } else { 3.0 };
                let noise =
                    (((k as u32).wrapping_mul(2_654_435_761).wrapping_add(seed) >> 16) as f32 / 65536.0 - 0.5) * 0.04;
                (if ph < 2000 { hi } else { 0.0 }) + noise
            })
            .collect()
    };
    let m = oos_dsp::mask::build(&wave(false, 1), 20, 0.2);
    let (lo, hi) = m.columns(mask::COLUMNS);
    // the record is the screen here: 20,000 samples at 10 MSa/s around the trigger
    let t = mask::Timing { points: n as u32, pre: n as u32 / 2, rate: 10_000_000, mask: 1, screen: (-1e-3, 2e-3) };
    let cols = mask::table_map(&lo, &hi, &t, scale, offset, 1.0, &mask::Map::default());
    // the read-out word of a sample (inverse of `seq::decode_volts`), and the 8-bit comparator on it
    let fails = |v: &[f32]| -> bool {
        v.iter().enumerate().any(|(k, &x)| {
            let w = (32768.0 + (x as f64 + offset) / scale * 7500.0).round().clamp(0.0, 65535.0) as u32;
            let c = (w >> mask::SHIFT) as u8;
            let col = cols[k * mask::COLUMNS / n];
            (col.lo_en && c < col.lo) || (col.hi_en && c > col.hi)
        })
    };
    for seed in 2..8 {
        let v = wave(false, seed);
        assert!(m.test(&v).pass && !fails(&v), "seed {}", seed);
    }
    let v = wave(true, 9);
    assert!(!m.test(&v).pass && fails(&v));
}
