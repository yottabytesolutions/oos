// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::board::{HwShared, Slow};
use oos_hal::dho::{bus::DmaBuf, seg::sim};
use oos_hal::sync::lock;

fn counts() -> [u64; 8] {
    crate::drawpipe::counters()
        .acquisition
        .text()
        .split_whitespace()
        .map(|field| field.split_once('=').unwrap().1.parse().unwrap())
        .collect::<Vec<_>>()
        .try_into()
        .unwrap()
}

fn settings() -> seq::Settings {
    seq::Settings { mask: 1, depth: 1024, frames: 1, ..Default::default() }
}

#[test]
fn configure_and_segment_operations_count_once_without_counting_completion() {
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let mut s = settings();
    s.segments = Some(seq::Segments { n: 4, interval: 0.0 });
    let mut state = seq::State::handover(s.mask, None);
    assert_eq!(counts(), [0; 8]);
    let p = configure(&mut state, &s, None, None).unwrap().1;
    assert_eq!(counts(), [0, 0, 0, 0, 0, 0, 1, 0]);
    let (_, mut bus, mut c2h) = sim::engine(sim::Engine::new(0, 1_000_000_000_000, 1, p.ratio));
    seg_start(&mut state, &s, &p, &mut bus, &mut c2h).unwrap();
    assert_eq!(counts(), [1, 0, 0, 0, 0, 0, 1, 0]);
    // Polling a recording does not count as another start or an abort.
    assert!(!seg::status(&mut bus, 4).complete(4));
    assert_eq!(counts(), [1, 0, 0, 0, 0, 0, 1, 0]);
    seg_stop(&mut state, &mut bus, &mut c2h, true).unwrap();
    assert_eq!(counts(), [1, 1, 0, 0, 0, 0, 1, 0]);
    seg_start(&mut state, &s, &p, &mut bus, &mut c2h).unwrap();
    assert_eq!(counts(), [2, 1, 0, 0, 0, 0, 1, 0]);
    for _ in 0..4 {
        seg::status(&mut bus, 4);
    }
    assert!(seg::status(&mut bus, 4).complete(4));
    seg_stop(&mut state, &mut bus, &mut c2h, false).unwrap();
    assert_eq!(counts(), [2, 1, 0, 0, 0, 0, 1, 0]);
    s.mask = 0;
    assert!(configure(&mut state, &s, None, None).is_err());
    assert_eq!(counts(), [2, 1, 0, 0, 0, 0, 2, 0], "failed calls count too");
}

#[test]
fn roll_poll_stops_once_for_hold_or_rearm_and_never_for_an_idle_stream() {
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let s = settings();
    let mut state = seq::State::handover(s.mask, None);
    let p = state.configure(&s, None, None).unwrap().1;
    let rp = roll::params(s.mask, seq::tdiv_fs(0.2), s.depth, &p);
    let (_, mut bus, _) = sim::engine(sim::Engine::new(0, 1_000_000_000_000, 1, p.ratio));
    let shared = HwShared::default();
    let mut ring = std::array::from_fn(|_| Default::default());
    let mut la = Default::default();
    for (hold, rearm) in [(true, false), (false, true)] {
        bus.apply(&roll_start(&mut state.sh, s.mask));
        let started = counts();
        let mut rolling = Some(rp);
        {
            let mut g = lock(&shared);
            (g.hold, g.rearm) = (hold, rearm);
        }
        // The same production poll drives the fake bus; repeated STOP polls are inert.
        for _ in 0..2 {
            assert_eq!(
                crate::board::dho::roll::poll(
                    Slow::Roll,
                    &shared,
                    &mut rolling,
                    &mut ring,
                    &mut la,
                    &mut state.sh,
                    &mut bus
                ),
                !hold
            );
            assert!(rolling.is_none());
            let mut expected = started;
            expected[2] += 1;
            assert_eq!(counts(), expected);
        }
    }
    assert_eq!(counts(), [0, 0, 2, 2, 0, 0, 0, 0]);
}

#[test]
fn prearm_disarm_and_single_count_hardware_arms_instead_of_reads() {
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let mut s = settings();
    let mut state = seq::State::handover(s.mask, None);
    let p = state.configure(&s, None, None).unwrap().1;
    let (_, mut bus, mut c2h) = sim::engine(sim::Engine::new(0, 1_000_000_000_000, 1, p.ratio));
    let mut dma = DmaBuf::new();
    // Host equivalent of seq::prearm: the exact steps with the bus executor.
    prearm(false, || seq::execute_bus(&state.arm_only_steps(&s, &p), &mut bus, &mut c2h, &mut dma, |_| {})).unwrap();
    assert_eq!(counts(), [0, 0, 0, 0, 1, 0, 0, 0]);
    disarm(|| seq::execute_bus(&state.disarm_steps(), &mut bus, &mut c2h, &mut dma, |_| {})).unwrap();
    assert_eq!(counts(), [0, 0, 0, 0, 1, 1, 0, 0]);
    for (single, ahead, expected_arms) in [(false, false, 0), (true, false, 1), (true, true, 2)] {
        s.single = single;
        if ahead {
            prearm(single, || seq::execute_bus(&state.arm_only_steps(&s, &p), &mut bus, &mut c2h, &mut dma, |_| {}))
                .unwrap();
            assert_eq!(counts(), [0, 0, 0, 0, 2, 1, 0, 2]);
        }
        let count = batch(single, ahead, || {
            seq::acquire_batch_idle(
                &mut state,
                &s,
                &p,
                &mut bus,
                &mut c2h,
                &mut dma,
                ahead,
                0,
                |_| {},
                || false,
                || false,
            )
        })
        .unwrap();
        assert_eq!(count, 1);
        assert_eq!(counts(), [0, 0, 0, 0, if ahead { 2 } else { 1 }, 1, 0, expected_arms]);
    }
    assert_eq!(prearm(true, || Err::<(), _>("arm failed")), Err("arm failed"));
    assert_eq!(counts(), [0, 0, 0, 0, 3, 1, 0, 3]);
    assert_eq!(disarm(|| Err::<(), _>("stop failed")), Err("stop failed"));
    assert_eq!(counts(), [0, 0, 0, 0, 3, 2, 0, 3]);
    assert_eq!(batch(true, false, || Err::<(), _>("read failed")), Err("read failed"));
    assert_eq!(counts(), [0, 0, 0, 0, 3, 2, 0, 4]);
    assert_eq!(batch(true, true, || Err::<(), _>("read failed")), Err("read failed"));
    assert_eq!(counts(), [0, 0, 0, 0, 3, 2, 0, 4]);
}
