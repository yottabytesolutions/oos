// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::dho::seq::{test_support::writes, Chan, Playback, HOLDOFF_MIN};
use crate::dho::Write;

#[test]
fn the_afe_sees_the_bnc_behind_a_probe() {
    let cal = crate::dho::fixtures::afe();
    let afe_ops = |k: Chan| {
        let s = Settings { afe: true, ch: [k; 4], ..Settings::default() };
        let (st, _) = plan(&mut State::handover(1, None), &s, Some(&cal), None).unwrap();
        st.iter().filter(|x| matches!(x, Step::Afe(_))).map(|x| format!("{:?}", x)).collect::<Vec<_>>()
    };
    let k = Settings::default().ch[0];
    let k = Chan { scale: 0.5, offset: 0.25, ..k };
    let tip = Chan { scale: 5.0, offset: 2.5, probe: 10.0, ..k };
    assert!(!afe_ops(k).is_empty());
    assert_eq!(afe_ops(k), afe_ops(tip));
    // the zero correction moves the AFE offset and nothing else
    assert_eq!(afe_ops(Chan { offset: 0.35, bias: 0.1, ..k }), afe_ops(k));
}

fn cal() -> afe::Cal {
    crate::dho::fixtures::afe()
}

fn afe_writes(st: &[Step], reg: u8) -> Vec<u16> {
    st.iter()
        .filter_map(|x| {
            if let Step::Afe(afe::Op::Afe { reg: r, val, .. }) = x {
                (*r == reg).then_some(*val)
            } else {
                None
            }
        })
        .collect()
}

#[test]
fn coupling_goes_to_the_afe_and_gnd_to_the_fpga() {
    let cal = cal();
    let mut st = State::handover(1, None);
    let k = Settings::default().ch[0];
    let run = |st: &mut State, couple: Couple| {
        let s = Settings { afe: true, ch: [Chan { couple, offset: 0.5, ..k }; 4], ..Settings::default() };
        st.configure(&s, Some(&cal), None).unwrap().0
    };
    let dc = run(&mut st, Couple::Dc);
    assert!(afe_writes(&dc, 0x07).iter().all(|v| v & 0x8000 != 0));
    let ac = run(&mut st, Couple::Ac);
    assert_eq!(afe_writes(&ac, 0x07).last().unwrap() & 0x8000, 0);
    // GND leaves the AFE alone and flattens the channel in the FPGA: 0.5 V at 1 V/div
    let g = run(&mut st, Couple::Gnd);
    assert_eq!(afe_writes(&g, 0x07).last().unwrap() & 0x8000, 0);
    let w = writes(&g);
    assert!(w.contains(&(0x14c8, 0)) && w.contains(&(0x14d0, 32768 + 3750)), "{:?}", &w[..4]);
    assert!(afe_writes(&run(&mut st, Couple::Dc), 0x07).last().unwrap() & 0x8000 != 0);
}

#[test]
fn invert_is_done_in_the_fpga_with_a_mirrored_offset() {
    let cal = cal();
    let k = Settings::default().ch[0];
    let run = |invert: bool, offset: f64| {
        let s = Settings { afe: true, ch: [Chan { invert, offset, ..k }; 4], ..Settings::default() };
        State::handover(1, None).configure(&s, Some(&cal), None).unwrap().0
    };
    let (a, b) = (run(false, 0.5), run(true, -0.5));
    assert_eq!(afe_writes(&a, 0x28), afe_writes(&b, 0x28));
    let inv = |st: &[Step]| writes(st).iter().rev().find(|w| w.0 == 0x1004).unwrap().1 >> 16 & 0xf;
    assert_eq!((inv(&a), inv(&b)), (0, 0xf));
}

#[test]
fn peak_mode_delay_and_holdoff_reach_the_hardware() {
    let peak = |mode: acq::Mode, tdiv: f64| {
        let s = Settings { mode, tdiv, depth: 100_000, ..Settings::default() };
        let st = State::handover(1, None).configure(&s, None, None).unwrap().0;
        writes(&st).iter().rev().find(|w| w.0 == 0x1004).unwrap().1 & 2
    };
    assert_eq!((peak(acq::Mode::Normal, 1e-3), peak(acq::Mode::Peak, 1e-3)), (0, 2));
    // the record follows the delay, and stops at half a record
    let pre = |delay: f64| {
        let s = Settings { delay, depth: 100_000, tdiv: 1e-6, ..Settings::default() };
        State::handover(1, None).configure(&s, None, None).unwrap().1
    };
    let (a, b) = (pre(0.0), pre(3e-6));
    assert_eq!(a.pre as i64 - b.pre as i64, (3e-6 * a.rate as f64) as i64);
    assert_eq!((pre(1.0).pre, pre(-1.0).pre), (0, a.memsel));
    // the auto time waits out a long holdoff (trigger.md 2.6): 1 s are 250 M ticks of the 4 ns timer
    let t2 = |holdoff: f64| {
        let s = Settings { holdoff, ..Settings::default() };
        writes(&State::handover(1, None).configure(&s, None, None).unwrap().0)
            .iter()
            .rev()
            .find(|w| w.0 == 0x4064)
            .unwrap()
            .1
    };
    assert_eq!((t2(HOLDOFF_MIN), t2(1.0)), (25_000_000, 250_000_000));
}

// Break it catches: the auto set-up's fast auto trigger not reaching the SCU, so at 20 ms/div every record of a
// signal the level does not cross waits 400 ms (docs/fork/trigger-lockin.md)
#[test]
fn fast_auto_trigger_takes_the_shortest_times() {
    let t = |auto_fast: bool| {
        let s = Settings {
            tdiv: 20e-3,
            trigger_options: crate::dho::seq::TriggerOptions::new(auto_fast, false),
            ..Settings::default()
        };
        let w = writes(&State::handover(1, None).configure(&s, None, None).unwrap().0);
        let last = |r: u32| w.iter().rev().find(|x| x.0 == r).unwrap().1;
        (last(0x4060), last(0x4064))
    };
    // 4 ns ticks: 400 ms / 200 ms at 20 ms/div, 10 ms / 100 ms fast
    assert_eq!(t(false), (100_000_000, 50_000_000));
    assert_eq!(t(true), (2_500_000, 25_000_000));
}

#[test]
fn aux_out_is_written_once_and_on_change() {
    use auxout::{AuxOut, PassFail};
    let aux = |st: &mut State, s: &Settings| -> Vec<Write> {
        writes(&st.configure(s, None, None).unwrap().0)
            .into_iter()
            .filter(|w| w.0 == auxout::SELECT || w.0 == auxout::PASSFAIL)
            .collect()
    };
    let mut st = State::handover(1, None);
    let base = Settings::default();
    assert_eq!(aux(&mut st, &base), vec![(0x1430, 0)], "the first set-up states the power-up source");
    assert_eq!(aux(&mut st, &base), vec![]);
    let off = Settings { aux: AuxOut::Off, ..base.clone() };
    assert_eq!(aux(&mut st, &off), vec![(0x1430, 2)]);
    assert_eq!(aux(&mut st, &off), vec![]);
    let pf = Settings { aux: AuxOut::PassFail(PassFail::DEFAULT), ..base.clone() };
    assert_eq!(aux(&mut st, &pf), vec![(0x119c, 156), (0x1430, 3)]);
    assert_eq!(aux(&mut st, &base), vec![(0x1430, 0)]);
    // the diagnostic arm without the acquisition block leaves AUX OUT alone
    let mut st = State::handover(1, None);
    assert_eq!(aux(&mut st, &Settings { blocks: 5, ..off }), vec![]);
}

#[test]
fn default_plan_ends_with_export_init_and_type4_play() {
    let mut state = State::handover(1, None);
    let (st, p) = plan(&mut state, &Settings::default(), None, None).unwrap();
    assert_eq!(p.memsel, 2_500_000);
    let w = writes(&st);
    assert!(w.contains(&(0x1108, 1_248_000)) && w.contains(&(0x1100, 0x11e8)));
    assert!(matches!(st.last(), Some(Step::Read { frames: 1, frame: 8000 })));
    // trigger 17, acquisition 22 + AUX OUT 1 (first set-up only), auto trigger 3, arm 29, pass-through 10, read-out
    // window 7 + stop sequence 17 + type-4 play 12 (the sequence that delivered own frames on the unit, 2026-11-03)
    assert_eq!(w.len(), 17 + 22 + 1 + 3 + 29 + 10 + 7 + 17 + 12);
    let k = w.iter().position(|x| *x == (0x1170, 0x4000_0001)).unwrap();
    assert!(k > 17 + 22 + 1 + 3 + 29 - 1, "pass-through comes after the arm");
    assert_eq!(*w.last().unwrap(), (0x4004, 0x1));
    // the play-last read-out of the warm hand-over is still available: 28 writes
    let mut state = State::handover(1, None);
    let (st, _) = plan(&mut state, &Settings { playback: Playback::Last, ..Settings::default() }, None, None).unwrap();
    assert_eq!(writes(&st).len(), 17 + 22 + 1 + 3 + 29 + 28);
}

#[test]
fn la_off_writes_nothing_new_and_la_on_adds_the_set_up_and_a_second_read() {
    // LA off: no LA register in the plan
    let mut state = State::handover(1, None);
    let s = Settings { tdiv: 1e-3, depth: 100_000, window: None, ..Settings::default() };
    let (st, p) = plan(&mut state, &s, None, None).unwrap();
    let la_regs = [0x128c, 0x12a4, 0x12a8, 0x12ac, 0x12c0, 0x12d4, 0x1298, 0x1280];
    assert!(!writes(&st).iter().any(|w| la_regs.contains(&w.0)));
    assert!(writes(&st).iter().all(|w| w.0 != 0x4008 || w.1 & (1 << 26) == 0));
    // LA on with CH1 shown: halved rate, doubled record registers, enable, rate block, thresholds, second read
    let mut state = State::handover(1, None);
    let sl = Settings { la: Some(la::Set::default()), frames: 50, ..s.clone() };
    let (st, pl) = plan(&mut state, &sl, None, None).unwrap();
    assert_eq!((pl.rate * 2, pl.memsel, pl.seg), (p.rate, p.memsel, 2 * p.seg));
    let w = writes(&st);
    assert!(
        w.contains(&(0x10e4, 2 * pl.memsel)) && w.contains(&(0x4050, 2 * pl.pre)) && w.contains(&(0x12d4, 0x2336_2353))
    );
    assert!(w.iter().any(|x| x.0 == 0x4008 && x.1 & (1 << 26) != 0));
    let t = la::timing(&pl, 1);
    assert!(w.contains(&la::rate_block(t.rate)[2]) && w.contains(&(0x1100, 10 * t.n)));
    // one frame per batch, the analog read, then the LA read of 2 x roundup4(n) bytes, then the stream restore
    assert_eq!(frames(&sl, &pl), 1);
    let reads: Vec<&Step> = st.iter().filter(|x| matches!(x, Step::Read { .. })).collect();
    assert_eq!(reads.len(), 2);
    assert!(matches!(reads[1], Step::Read { frames: 1, frame } if *frame == la::read_bytes(t.n)));
    assert_eq!(state.sh.s4008 & (1 << 23 | 1 << 24 | 1 << 26), 1 << 23 | 1 << 26);
    // LA alone (no analog channel shown): the rate is not halved
    let mut state = State::handover(1, None);
    let (_, pa) = plan(
        &mut state,
        &Settings { la: Some(la::Set { analog_shown: false, ..la::Set::default() }), ..s },
        None,
        None,
    )
    .unwrap();
    assert_eq!(pa.rate, p.rate);
}

#[test]
fn channel_set_change_switches_the_adc_once() {
    let s = Settings { mask: 3, ..Settings::default() };
    let mut state = State::handover(1, None);
    assert!(state.configure(&s, None, None).is_err());
    let cal = crate::dho::fixtures::adc();
    let mut state = State::handover(1, Some(&cal));
    let (st, _) = state.configure(&s, None, Some(&cal)).unwrap();
    let w = writes(&st);
    assert_eq!(&w[..2], &[(0x1034, 0x0154_1860), (0x1034, 0x0155_0005)]);
    // second configure with the same set: no ADC writes
    let (st2, _) = state.configure(&s, None, Some(&cal)).unwrap();
    assert!(!writes(&st2).iter().any(|w| w.0 == 0x1034));
}
