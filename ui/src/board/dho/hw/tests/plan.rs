// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

// Break it catches: Auto depth that ignores the trigger delay, so the record (centred on the delay, the trigger
// inside it) ends before the screen does: 10 us/div with 200 us delay drew 20 us of the screen (unit 2026-11-13)
#[test]
fn auto_depth_covers_a_delayed_screen() {
    for (tdiv, delay, chans) in
        [(10e-6f32, 200e-6f32, 1usize), (10e-6, -200e-6, 1), (1e-6, 50e-6, 2), (100e-6, 3e-3, 4), (10e-6, 0.0, 1)]
    {
        let mut m = Model {
            tscale: tscales().iter().position(|&t| (t - tdiv).abs() < tdiv * 1e-3).unwrap(),
            delay,
            ..Default::default()
        };
        for c in 0..NCH {
            m.ch[c].on = c < chans;
        }
        let s = settings(&m, false, &Default::default());
        let mut st = oos_hal::dho::seq::State::handover(s.mask, None);
        let (_, p) = st.configure(&s, None, None).unwrap();
        // the record's span relative to the trigger: [first, last] in s
        let dot = p.dot_fs as f64 * 1e-15;
        let (first, last) = (-(p.pre as f64) * dot, p.post as f64 * dot);
        let (lo, hi) = (delay as f64 - 5.0 * tdiv as f64, delay as f64 + 5.0 * tdiv as f64);
        assert!(
            first <= lo + dot && last >= hi - dot,
            "{} s/div delay {} {} ch: record {}..{} s, screen {}..{}",
            tdiv,
            delay,
            chans,
            first,
            last,
            lo,
            hi
        );
        if delay == 0.0 {
            assert!(s.depth <= LIVE_DEPTH, "no delay: the plain Auto depth");
        }
    }
}

#[test]
fn aux_out_reaches_the_settings() {
    let mut m = Model::default();
    assert_eq!(settings(&m, false, &Default::default()).aux, auxout::AuxOut::TriggerOut);
    m.aux_out = AuxOut::Off;
    let s = settings(&m, false, &Default::default());
    assert_eq!(s.aux, auxout::AuxOut::Off);
    let w = oos_hal::dho::seq::State::handover(s.mask, None).configure(&s, None, None).unwrap().0;
    assert!(w.iter().any(|x| matches!(x, oos_hal::dho::seq::Step::Wr((0x1430, 2)))));
    m.aux_out = AuxOut::PassFail;
    assert_eq!(settings(&m, false, &Default::default()).aux, auxout::AuxOut::PassFail(auxout::PassFail::DEFAULT));
}

#[test]
fn mask_columns_of_the_board() {
    assert_eq!(crate::board::MASK_COLUMNS, oos_hal::dho::mask::COLUMNS);
}
