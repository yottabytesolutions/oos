// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

#[test]
fn dvm_hw_from_conf() {
    assert!(!dvm_hw_from(""));
    assert!(dvm_hw_from("dvm.hw = on # FPGA DVM\n"));
    assert!(!dvm_hw_from("dvm.hw = yes\n"));
}

#[test]
fn dvm_steps() {
    let t0 = Some((1, 2, 7));
    let ch = |c: usize| Some(crate::board::DvmWant { ch: c, lane: None });
    // (want, mask, config, running, elapsed) -> (step, running after)
    let cases = [
        (None, 0b11, 7, None, 0.0, DvmStep::Idle, None),
        // CH2 of CH1+CH2: lane 2
        (ch(1), 0b11, 7, None, 0.0, DvmStep::Start(2), t0),
        (ch(1), 0b11, 7, t0, 0.1, DvmStep::Idle, t0),
        (ch(1), 0b11, 7, t0, 0.6, DvmStep::Read, t0),
        // a new configuration restarts it
        (ch(1), 0b11, 8, t0, 0.1, DvmStep::Start(2), Some((1, 2, 8))),
        // CH2 alone: lane 1; with lane 3 forced: lane 3
        (ch(1), 0b10, 7, t0, 0.6, DvmStep::Start(1), Some((1, 1, 7))),
        (
            Some(crate::board::DvmWant { ch: 1, lane: Some(3) }),
            0b10,
            7,
            Some((1, 1, 7)),
            0.6,
            DvmStep::Start(3),
            Some((1, 3, 7)),
        ),
        // the channel left the acquired set, or the DVM was switched off
        (ch(1), 0b01, 7, t0, 0.6, DvmStep::Stop, None),
        (None, 0b11, 7, t0, 0.6, DvmStep::Stop, None),
    ];
    for (want, mask, config, running, el, step, after) in cases {
        assert_eq!(
            dvm_step(want, mask, config, running, el),
            (step, after),
            "{:?} {:#b} {} {:?} {}",
            want,
            mask,
            config,
            running,
            el
        );
    }
}

// Round 15: a trim right after the start-up mode switches read a settling front end. Break it catches: a trim
// that is due before the configuration has stood for TRIM_HOLD.
#[test]
fn core_trim_waits_for_a_standing_configuration() {
    let t0 = std::time::Instant::now();
    let p = TrimPending { mode: 2, mask: 0b100, since: t0 };
    assert!(!p.due(t0) && !p.due(t0 + TRIM_HOLD - std::time::Duration::from_millis(1)));
    assert!(p.due(t0 + TRIM_HOLD) && TRIM_HOLD >= std::time::Duration::from_secs(1));
}

#[test]
fn core_offset_from_conf() {
    assert!(!core_offset_from(""));
    assert!(core_offset_from("cal.core_offset = tight\n"));
    assert!(!core_offset_from("cal.core_offset = on\n"));
}

#[test]
fn prearm_from_env_or_conf() {
    assert!(!prearm_from(false, ""));
    assert!(prearm_from(true, ""));
    assert!(prearm_from(true, "acq.prearm = off\n"));
    assert!(prearm_from(false, "ntp = off\nacq.prearm = on   # arm ahead\n"));
    assert!(!prearm_from(false, "acq.prearm = yes\n"));
}

#[test]
fn slow_modes_follow_auto_roll() {
    let mut m = Model::default();
    let at = |m: &mut Model, t: f64| {
        m.tscale = tscales().iter().position(|&x| (x as f64 - t).abs() < t * 1e-3).unwrap();
        slow_mode(m)
    };
    assert_eq!(at(&mut m, 0.1), Slow::Off);
    assert_eq!(at(&mut m, 0.2), Slow::Roll);
    m.auto_roll = false;
    assert_eq!(at(&mut m, 0.1), Slow::Off);
    assert_eq!(at(&mut m, 0.2), Slow::Scan { delay: 0.0, single: false });
    m.trig_mode = TrigMode::Single;
    assert_eq!(at(&mut m, 0.5), Slow::Scan { delay: 0.0, single: true });
    // averaging: no scan
    m.acq = AcqMode::Average;
    assert_eq!(at(&mut m, 0.5), Slow::Off);
    // roll and scan stream with the stock Auto depth
    m.acq = AcqMode::Normal;
    assert_eq!(settings(&m, false, &Default::default()).depth, 0);
}
