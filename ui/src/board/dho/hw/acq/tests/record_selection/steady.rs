// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use oos_core::change::{Request as Change, Shadow, UiAcq};

mod boundaries;
mod readout;

fn selections(result: &Outcome, deposit: bool) -> Vec<u16> {
    if deposit {
        deposit::submissions(result).iter().flat_map(|batch| batch.first_words()).map(|word| word >> 8).collect()
    } else {
        result.queue.iter().map(|frame| frame.code() >> 8).collect()
    }
}

fn repeated(case: &Case, deposit: bool, conf: &str) -> [Outcome; 2] {
    twice(|| {
        fixture::series(case, deposit, conf, |source, io, trace| {
            std::array::from_fn(|_| {
                fixture::dispatch(case, source, io);
                fixture::observed(source, io, trace, deposit)
            })
        })
    })
}

#[test]
fn steady_batches_use_slot_zero_after_the_first_read() {
    let case = Case { signal: Signal::Both, ..Case::new(2) };
    for deposit in [false, true] {
        let [first, second] = repeated(&case, deposit, "");
        assert_eq!(selections(&first, deposit), [1]);
        assert_eq!(selections(&second, deposit), [0, 1]);
        assert_eq!(second.counters.loss("lost.frame0"), 1);
    }
}

#[test]
fn rejected_last_slot_leaves_picture_and_readback_on_the_previous_batch() {
    let first = Case { signal: Signal::Both, ..Case::new(2) };
    let next = Case { signal: Signal::First, ..Case::new(2) };
    for deposit in [false, true] {
        let (before, rejected) = twice(|| {
            fixture::series(&first, deposit, "", |source, io, trace| {
                fixture::dispatch(&first, source, io);
                let before = fixture::observed(source, io, trace, deposit);
                fixture::dispatch(&next, source, io);
                (before, fixture::observed(source, io, trace, deposit))
            })
        });
        assert_newest(&before, deposit, 2, 1);
        assert!(selections(&rejected, deposit).is_empty());
        assert!(deposit::submissions(&rejected).is_empty());
        assert_eq!(rejected.latest, before.latest);
        for event in &rejected.events {
            if let Event::Picture(picture) = event {
                assert_eq!(picture.frames, 0);
                assert!(picture.tap.is_none());
            }
        }
        assert_eq!(rejected.counters.deposited, u64::from(deposit));
        assert_eq!(rejected.counters.loss("lost.frame0"), 2);
        assert_eq!(rejected.counters.loss("lost.frame1"), 1);
    }
}

#[test]
fn slot_zero_mask_violation_is_counted_with_the_other_records() {
    let mut case = Case { signal: Signal::Edge, ..Case::new(2) };
    case.settings.trig_couple = tpu::Coupling::Ac;
    let [first, steady] = fixture::series(&case, false, "", |source, io, _| {
        std::array::from_fn(|_| {
            fixture::dispatch(&case, source, io);
            std::mem::take(&mut lock(&source.out).queue)
        })
    });
    let slots = |queue: &std::collections::VecDeque<std::sync::Arc<crate::board::Frame>>| {
        queue.iter().map(|frame| Frame::read(frame).code() >> 8).collect::<Vec<_>>()
    };
    assert_eq!(slots(&first), [1]);
    assert_eq!(slots(&steady), [0, 1]);

    let shared = HwShared::default();
    let mut ui = crate::ui::Ui::for_tests(crate::wave::Source::Hw { shared: shared.clone(), seen: 0 });
    ui.m = picture::model(case.settings.tdiv);
    ui.m.mask_x = 0.0;
    ui.m.mask_y = 0.01;
    let step = |queue: std::collections::VecDeque<std::sync::Arc<crate::board::Frame>>, ui: &mut crate::ui::Ui| {
        let mut target = lock(&shared);
        let id = target.acquisition.identity();
        target.queue.extend(queue.into_iter().map(|mut owned| {
            let frame = std::sync::Arc::make_mut(&mut owned);
            (frame.stamp, frame.cfg) = (id.epoch(), id.gen());
            owned
        }));
        drop(target);
        ui.step(0.001);
    };
    ui.step(0.001);
    step(first, &mut ui);
    assert!(ui.mask_create());
    ui.step(0.001);
    step(steady, &mut ui);
    assert_eq!(ui.mask_counts(), (1, 1));
    assert_eq!(ui.records, 3);
}

#[test]
fn steady_slot_zero_survives_a_hole_before_the_newest_record() {
    let case = Case { signal: Signal::First, ..Case::new(3) };
    for deposit in [false, true] {
        let [first, second] = repeated(&case, deposit, "");
        assert_eq!(selections(&first, deposit), [2]);
        assert_eq!(selections(&second, deposit), [0, 2]);
        assert_eq!(second.counters.loss("lost.frame0"), 1);
        assert_eq!(second.counters.loss("lost.frame1"), 2);
        assert_newest(&second, deposit, 6, 2);
    }
}

#[test]
fn proof_alone_does_not_keep_slot_zero_without_its_edge() {
    let case = Case { signal: Signal::Edge, ..Case::new(2) };
    for deposit in [false, true] {
        let [first, second] = repeated(&case, deposit, "");
        assert_eq!(selections(&first, deposit), [1]);
        assert_eq!(selections(&second, deposit), [1]);
        assert_eq!(second.counters.loss("lost.frame0"), 2);
        assert_newest(&second, deposit, 4, 1);
    }
}

#[test]
fn keep_and_drop_override_proof_and_keep_bypasses_edge_checks() {
    let case = Case::new(2);
    for deposit in [false, true] {
        for (value, slots, losses) in [("acq.frame0 = keep", &[0, 1][..], 0), ("acq.frame0 = drop", &[], 2)] {
            let [first, second] = repeated(&case, deposit, value);
            assert_eq!(selections(&first, deposit), slots);
            assert_eq!(selections(&second, deposit), slots);
            assert_eq!(second.counters.loss("lost.frame0"), losses);
            assert_eq!(second.counters.loss("lost.frame1"), losses);
            if losses == 0 {
                assert_newest(&second, deposit, 4, 1);
            }
        }
    }
}

#[test]
fn short_dma_drops_slot_zero_and_cannot_prove_the_next_arm() {
    let case = Case { signal: Signal::Both, ..Case::new(3) };
    for deposit in [false, true] {
        let outcomes = twice(|| {
            fixture::series(&case, deposit, "", |source, io, trace| {
                [&case, &Case { available: 2, signal: Signal::Both, ..Case::new(3) }, &case, &case].map(|trial| {
                    fixture::dispatch(trial, source, io);
                    fixture::observed(source, io, trace, deposit)
                })
            })
        });
        assert_eq!(
            outcomes.each_ref().map(|result| selections(result, deposit)),
            [vec![1, 2], vec![1], vec![1, 2], vec![0, 1, 2]]
        );
        assert_eq!(outcomes[3].counters.loss("lost.error"), 1);
        assert_eq!(outcomes[3].counters.loss("lost.frame0"), 3);
        assert_newest(&outcomes[1], deposit, 5, 1);
    }
}

fn assert_newest(result: &Outcome, deposit: bool, seq: u64, slot: u16) {
    if deposit {
        assert_eq!(deposit::submissions(result)[0].newest_seq(), Some(seq));
        let picture = result
            .events
            .iter()
            .find_map(|event| if let Event::Picture(picture) = event { Some(picture) } else { None })
            .unwrap();
        assert_eq!(picture.tap.as_ref().map(|tap| &tap.frame), Some(&Frame::stamp(seq, slot)));
    } else {
        assert_eq!(result.latest, Some(Frame::stamp(seq, slot)));
    }
}
