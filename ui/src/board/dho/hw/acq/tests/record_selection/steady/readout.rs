// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::board::{MaskWant, Shared, ZoneWant, MASK_COLUMNS};
use oos_hal::dho::mask;

#[test]
fn live_raw_replay_invalidates_the_successor_only() {
    raw_replay(false);
}

#[test]
fn failed_raw_replay_invalidates_current_and_successor() {
    raw_replay(true);
}

fn raw_replay(fail: bool) {
    let mut case = Case::new(2);
    case.settings.compress = 4;
    case.settings.tdiv = 10e-6;
    case.settings.depth = 14_000;
    case.settings.trig_couple = tpu::Coupling::Ac;
    let outcomes = twice(|| {
        fixture::series(&case, false, "", |source, io, trace| {
            std::array::from_fn::<_, 5, _>(|pass| {
                if pass == 2 {
                    io.clock.advance(crate::compress::RAW_PERIOD);
                }
                fixture::dispatch_with(&case, source, io, |_, stream| {
                    if pass == 2 && fail {
                        stream.fail_next();
                    }
                });
                fixture::observed(source, io, trace, false)
            })
        })
    });
    let slots = outcomes.each_ref().map(|result| selections(result, false));
    assert_eq!(slots, [vec![1], vec![1], if fail { vec![1] } else { vec![0, 1] }, vec![1], vec![0, 1]]);
    for (i, outcome) in outcomes.iter().enumerate() {
        assert_newest(outcome, false, 2 * (i as u64 + 1), 1);
        let replays = outcome.events.iter().filter(|event| matches!(event, Event::Dma(_))).count();
        assert_eq!(replays, 1 + usize::from(i == 0 || (i == 2 && !fail)));
    }
}

#[test]
fn logic_read_switches_streams_after_each_singleton() {
    let mut case = Case::new(1);
    case.settings.la = Some(Default::default());
    case.settings.trig_couple = tpu::Coupling::Ac;
    let outcomes = twice(|| {
        fixture::series(&case, false, "", |source, io, trace| {
            std::array::from_fn::<_, 3, _>(|_| {
                fixture::dispatch(&case, source, io);
                fixture::observed(source, io, trace, false)
            })
        })
    });
    for (i, outcome) in outcomes.iter().enumerate() {
        assert!(outcome.events.iter().any(|event| matches!(event, Event::Write(0x12c0, _))));
        assert_eq!(outcome.events.iter().filter(|event| matches!(event, Event::Dma(_))).count(), 2);
        if i > 0 {
            assert_eq!(selections(outcome, false), [0]);
            assert_eq!(outcome.latest.as_ref(), outcome.queue.last());
            assert_eq!(outcome.latest.as_ref().unwrap().seq(), i as u64 + 1);
        }
    }
}

fn check_latch<T>(configure: impl Fn(&mut Shared), running: impl Fn(&mut seq::State) -> &mut Option<T>) {
    let _map = crate::board::dho::hw::mask_map_test_lock();
    let case = Case { signal: Signal::Both, ..Case::new(2) };
    let outcomes = twice(|| {
        fixture::series(&case, false, "zone.batch = 2", |source, io, trace| {
            configure(&mut lock(&source.out));
            let mut inactive = None;
            std::array::from_fn::<_, 5, _>(|pass| {
                fixture::dispatch_with(&case, source, io, |state, _| {
                    // Schedule one latch at the readout seam without a settings change or a disarm.
                    if matches!(pass, 0 | 2 | 3) {
                        std::mem::swap(running(state), &mut inactive);
                    }
                });
                fixture::observed(source, io, trace, false)
            })
        })
    });
    let slots = outcomes.each_ref().map(|outcome| selections(outcome, false));
    assert_eq!(slots, [vec![1], vec![0, 1], vec![0, 1], vec![1], vec![0, 1]]);
    for (i, outcome) in outcomes.iter().enumerate() {
        let latch = outcome
            .events
            .iter()
            .position(|event| matches!(event, Event::Write(mask::CTRL, value) if value & mask::ctrl::LATCH != 0));
        assert_eq!(latch.is_some(), i == 2);
        if let Some(latch) = latch {
            assert!(outcome.events[..latch].iter().any(|event| matches!(event, Event::Dma(_))));
        }
    }
}

#[test]
fn mask_latch_invalidates_the_successor_only() {
    check_latch(
        |shared| {
            shared.mask_want = Some(MaskWant {
                ch: 0,
                lo: vec![-1.0; MASK_COLUMNS],
                hi: vec![1.0; MASK_COLUMNS],
                stop: false,
                clear: 0,
            });
        },
        |state| &mut state.mask_run,
    );
}

#[test]
fn zone_latch_invalidates_the_successor_only() {
    check_latch(
        |shared| {
            shared.zone_want =
                Some(ZoneWant { ch: 0, zones: vec![(vec![(-1e-7, -1.0), (1e-7, 1.0)], true)], clear: 0 });
        },
        |state| &mut state.zone_run,
    );
}

#[test]
fn deep_raw_read_invalidates_the_successor_only() {
    let mut case = Case::new(2);
    case.settings.depth = 2_500_000;
    case.settings.tdiv = 1e-3;
    case.settings.trig_couple = tpu::Coupling::Ac;
    let outcomes = twice(|| {
        fixture::series(&case, false, "", |source, io, trace| {
            std::array::from_fn::<_, 5, _>(|pass| {
                if pass == 2 {
                    lock(&source.out).deep_raw = Some(crate::deepmeas::Want { every: false });
                }
                fixture::dispatch(&case, source, io);
                fixture::observed(source, io, trace, false)
            })
        })
    });
    let slots = outcomes.each_ref().map(|outcome| selections(outcome, false));
    assert_eq!(slots, [vec![1], vec![0, 1], vec![0, 1], vec![1], vec![0, 1]]);
    for (i, outcome) in outcomes.iter().enumerate() {
        let reads = outcome.events.iter().filter(|event| matches!(event, Event::Dma(_))).count();
        assert_eq!(reads > 1, i == 2);
        assert_newest(outcome, false, 2 * (i as u64 + 1), 1);
    }
}
