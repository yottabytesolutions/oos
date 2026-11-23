// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::dho::seq::test_support::writes;

#[test]
fn hysteresis_follows_the_table_below_2_mv() {
    let h = |scale: f64, probe: f64| tpu::noise_hysteresis(scale / probe * 1e9, false);
    assert_eq!(
        (h(1.0, 1.0), h(0.002, 1.0), h(0.00204, 1.0), h(0.001, 1.0), h(0.0005, 1.0), h(0.0002, 1.0), h(0.0001, 1.0)),
        (0.3, 0.5, 0.3, 1.0, 1.25, 1.15, 2.3)
    );
    assert_eq!(h(0.01, 10.0), 1.0, "the BNC scale counts");
    assert_eq!(h(0.002f32 as f64, 1.0), 0.5, "through f32");
    assert_eq!(tpu::noise_hysteresis(1e9, true), 0.7);
}

#[test]
fn holdoff_is_written_when_it_matters() {
    let mut st = State::handover(1, None);
    let has = |w: &[crate::dho::Write]| w.iter().any(|x| x.0 == 0x2084);
    let base = Settings {
        trigger: Trig::Pulse { pol: tpu::Polarity::Pos, when: tpu::When::Gt, lower: 1e-6, upper: 2e-6 },
        ..Settings::default()
    };
    assert!(!has(&steps(&mut st, &base)));
    let on = Settings { holdoff: 1e-3, ..base.clone() };
    assert!(has(&steps(&mut st, &on)));
    assert!(has(&steps(&mut st, &base)), "back to none must reach the hardware");
    assert!(!has(&steps(&mut st, &base)));
}

#[test]
fn uart_trigger_is_written_with_the_trigger_block() {
    use crate::dho::uarttrig::{Cond, Parity, Stop, Uart};
    let u = Uart { baud: 115_200.0, bits: 8, parity: Parity::None, stop: Stop::One, cond: Cond::Start, invert: false };
    let s =
        Settings { trig_src: 2, mask: 0b11, slope: tpu::Slope::Falling, trigger: Trig::Uart(u), ..Settings::default() };
    let mut st = State::handover(0b11, None);
    let w = writes(&st.configure(&s, None, None).unwrap().0);
    let k = w.iter().position(|x| *x == (0x205c, 4)).expect("mux on the UART event");
    assert!(w[..k].contains(&(0x2100, 1356)) && w[..k].contains(&(0x2104, 7)));
    // the trigger block comes before the FSM stop poll and the acquisition block, like every trigger type
    assert!(k < w.iter().position(|x| x.0 == 0x4020).unwrap());
}

#[test]
fn every_hardware_trigger_type_configures() {
    use tpu::*;
    let pats = [Pat::H, Pat::X, Pat::X, Pat::Rise];
    for t in [
        Trig::Edge,
        Trig::Pulse { pol: Polarity::Pos, when: When::Lt, lower: 0.0, upper: 20e-6 },
        Trig::Slope { when: When::Gt, lower: 1e-6, upper: 0.0, lo: 0.5 },
        Trig::Runt { pol: Polarity::Pos, when: When::None, lower: 0.0, upper: 0.0, lo: 0.5 },
        Trig::Window { event: WindowEvent::Enter, time: 0.0, lo: -0.5 },
        Trig::Timeout { time: 1.2e-3 },
        Trig::Delay { srcb: 4, eb: Edge::Rising, when: When::Gt, lower: 40e-6, upper: 0.0, levelb: 1.65 },
        Trig::Pattern { pats, la: [Pat::X; 16], levels: [1.25, 0.0, 1.65, 1.65] },
        Trig::Duration {
            pats: [Pat::H, Pat::X, Pat::X, Pat::X],
            la: [Pat::X; 16],
            when: When::Gt,
            lower: 200e-6,
            upper: 0.0,
            levels: [1.25; 4],
        },
        Trig::SetupHold { clk: 4, clk_edge: Edge::Rising, setup: 2e-6, hold: 2e-6, levelc: 1.65 },
        Trig::Nth { count: 3, idle: 50e-6 },
        Trig::Video { std: VideoStd::Pal, sync: VideoSync::OddField, line: 1 },
    ] {
        let s = Settings { mask: 0xf, trigger: t.clone(), ..Settings::default() };
        let mut state = State::handover(0xf, None);
        let (st, _) = state.configure(&s, None, None).unwrap();
        // every type ends with the type select in 0x205c[4:0] written
        assert!(writes(&st).iter().any(|w| w.0 == 0x205c), "{:?}", t);
    }
    // Nth edge: ABR flags (1,1,1,0,1) + type 3 (trigger.md 10.11)
    let s = Settings { trigger: Trig::Nth { count: 3, idle: 50e-6 }, ..Settings::default() };
    let mut state = State::handover(1, None);
    let (st, _) = state.configure(&s, None, None).unwrap();
    assert_eq!(writes(&st).iter().rev().find(|w| w.0 == 0x205c).unwrap().1 & 0x7df, 0x5c3);
}
