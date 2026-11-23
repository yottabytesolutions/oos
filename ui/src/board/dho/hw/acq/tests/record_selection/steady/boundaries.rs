// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::{
    board::identity::Publication,
    model::{AcqMode, Model},
};

fn check(
    case: &Case,
    deposit: bool,
    boundary: impl Fn(&mut AcqLoop<FixedPacing>, &mut Io<sim::Sim>) -> Option<Case>,
) -> [Outcome; 4] {
    let outcomes = twice(|| {
        fixture::series(case, deposit, "", |source, io, trace| {
            let batch = |case: &Case, source: &mut AcqLoop<FixedPacing>, io: &mut Io<sim::Sim>| {
                fixture::dispatch(case, source, io);
                fixture::observed(source, io, trace, deposit)
            };
            let first = batch(case, source, io);
            let warm = batch(case, source, io);
            let next = boundary(source, io);
            let next = next.as_ref().unwrap_or(case);
            [first, warm, batch(next, source, io), batch(next, source, io)]
        })
    });
    let slots = outcomes.each_ref().map(|result| {
        if deposit {
            selections(result, true)
        } else {
            let newest = result.latest.as_ref().unwrap().seq();
            assert_eq!(result.queue.last().unwrap().seq(), newest);
            result.queue.iter().map(|frame| (frame.seq() - (newest - 1)) as u16).collect()
        }
    });
    assert_eq!(slots, [vec![1], vec![0, 1], vec![1], vec![0, 1]]);
    assert_eq!(outcomes[0].counters.loss("lost.frame0"), 1);
    assert_eq!(outcomes[1].counters.loss("lost.frame0"), 1);
    assert_eq!(outcomes[3].counters.loss("lost.frame0"), 2);
    outcomes
}

fn publish(source: &AcqLoop<FixedPacing>, settings: &Settings, identity: Identity) {
    let mut g = lock(&source.out);
    let deposit = g.acquisition.route();
    g.want = Some(settings.clone());
    g.acquisition = Publication::new(identity, deposit);
    if let Some(pipe) = drawpipe::pipe() {
        let (w, h) = picture::SIZE;
        pipe.publish_view(&picture::model(settings.tdiv), w, h, source.switches.interp, identity);
    }
}

fn epoch(change: Change) {
    let case = Case { signal: Signal::Both, ..Case::new(2) };
    for deposit in [false, true] {
        check(&case, deposit, |source, _| {
            let m = Model::default();
            let config = crate::model::config();
            let mut shadow = Shadow::new(&m, UiAcq::default(), &config).unwrap();
            let identity = shadow.observe(&m, UiAcq::default(), &[change], &config).unwrap().identity();
            assert_ne!(identity.epoch(), shadow_before(source).epoch());
            assert_eq!(identity.gen(), shadow_before(source).gen());
            publish(source, &case.settings, identity);
            None
        });
    }
}

fn shadow_before(source: &AcqLoop<FixedPacing>) -> Identity {
    lock(&source.out).acquisition.identity()
}

#[test]
fn run_edge_clears_proof() {
    epoch(Change::Run);
}

#[test]
fn clear_clears_proof() {
    epoch(Change::Clear);
}

#[test]
fn stop_clears_proof_even_without_an_epoch_change() {
    let case = Case { signal: Signal::Both, ..Case::new(2) };
    for deposit in [false, true] {
        check(&case, deposit, |source, io| {
            io.clock.stop();
            let request = Request::read(&lock(&source.out));
            assert!(source.reconfigure(io, &request, &mut false).is_none());
            io.clock.release();
            None
        });
    }
}

#[test]
fn calibration_clears_proof_before_any_held_poll() {
    let case = Case { signal: Signal::Both, ..Case::new(2) };
    check(&case, false, |source, io| {
        {
            let mut g = lock(&source.out);
            g.hold = true;
            g.cal_req = Some((vec![0], true, false));
        }
        assert!(source.service(io, None).is_none());
        lock(&source.out).hold = false;
        None
    });
}

#[test]
fn applied_reconfiguration_clears_proof_even_without_a_new_ui_generation() {
    let case = Case { signal: Signal::Both, ..Case::new(2) };
    check(&case, false, |source, _| {
        let mut next = Case { signal: Signal::Both, ..Case::new(2) };
        next.settings.ch[0].scale *= 1.2;
        lock(&source.out).want = Some(next.settings.clone());
        Some(next)
    });
}

#[test]
fn disarm_without_a_retained_batch_clears_proof() {
    let case = Case { signal: Signal::Both, ..Case::new(2) };
    check(&case, false, |source, io| {
        assert!(source.replay.last_batch.is_none());
        io.disarm(&mut source.batch, &mut source.replay, &source.out);
        None
    });
}

#[test]
fn discarded_retained_batch_clears_proof() {
    let mut case = Case::new(2);
    case.settings.trig_couple = tpu::Coupling::Ac;
    check(&case, false, |source, io| {
        source.switches.board.prearm = true;
        super::super::super::capture(source, io);
        assert!(source.batch.armed.is_some() && source.replay.last_batch.is_some());
        lock(&source.out).queue.clear();
        io.disarm(&mut source.batch, &mut source.replay, &source.out);
        assert!(source.replay.last_batch.is_none());
        source.switches.board.prearm = false;
        None
    });
}

#[test]
fn cancelled_fill_clears_proof() {
    let case = Case { signal: Signal::Both, ..Case::new(2) };
    check(&case, false, |source, io| {
        let out = source.out.clone();
        io.clock.stop_triggers();
        io.clock.drive(move |clock, _| {
            if clock.capture_armed() {
                lock(&out).force = true;
            }
        });
        let seq = source.seq;
        super::super::super::capture(source, io);
        assert_eq!(source.seq, seq, "the interrupted arm never reached dispatch");
        io.clock.drive(|_, _| {});
        io.clock.resume_triggers(2);
        lock(&source.out).force = false;
        None
    });
}

#[test]
fn dma_read_error_clears_proof() {
    let case = Case { signal: Signal::Both, ..Case::new(2) };
    check(&case, false, |source, io| {
        io.c2h.fail_next();
        let seq = source.seq;
        super::super::super::capture(source, io);
        assert_eq!(source.seq, seq, "the failed read never reached dispatch");
        None
    });
}

type Edit = fn(&mut Model, &mut Settings);

fn settings_edit(mask: u32, edit: Edit) {
    let mut case = Case::new(2);
    case.settings.mask = mask;
    case.settings.trig_couple = tpu::Coupling::Ac;
    for deposit in [false, true] {
        check(&case, deposit, |source, _| {
            let mut model = picture::model(case.settings.tdiv);
            model.ch[1].on = mask & 2 != 0;
            let config = crate::model::config();
            let mut shadow = Shadow::new(&model, UiAcq::default(), &config).unwrap();
            let mut next = Case { settings: case.settings.clone(), ..Case::new(2) };
            edit(&mut model, &mut next.settings);
            let identity = shadow.observe(&model, UiAcq::default(), &[], &config).unwrap().identity();
            assert_ne!(identity.gen(), shadow_before(source).gen());
            publish(source, &next.settings, identity);
            Some(next)
        });
    }
}

#[test]
fn vertical_edit_drops_once() {
    settings_edit(1, |m, s| {
        m.ch[0].vscale += 1;
        s.ch[0].scale *= 2.0;
    });
}

#[test]
fn timebase_edit_drops_once() {
    settings_edit(1, |m, s| {
        m.tscale += 1;
        s.tdiv *= 2.0;
    });
}

#[test]
fn trigger_level_edit_drops_once() {
    settings_edit(1, |m, s| {
        m.trig_level += 0.1;
        s.level += 0.1;
    });
}

#[test]
fn trigger_source_edit_drops_once() {
    settings_edit(3, |m, s| {
        m.trig_src = 1;
        s.trig_src = 2;
    });
}

#[test]
fn channel_enable_and_disable_each_drop_once() {
    for mask in [1, 3] {
        settings_edit(mask, |m, s| {
            m.ch[1].on = !m.ch[1].on;
            s.mask ^= 2;
        });
    }
}

#[test]
fn depth_edit_drops_once() {
    settings_edit(1, |m, s| {
        m.mdepth += 1;
        s.depth *= 2;
    });
}

#[test]
fn acquisition_mode_edit_drops_once() {
    settings_edit(1, |m, s| {
        m.acq = AcqMode::Peak;
        s.mode = Mode::Peak;
    });
}

#[test]
fn due_adc_core_trim_drops_slot_zero_once() {
    let case = Case { signal: Signal::Both, ..Case::new(2) };
    let [_, _, trimmed, _] = check(&case, false, |source, io| {
        source.units.configured(io.state.mode.min(10), case.settings.mask, true, io.clock.now());
        io.clock.advance(crate::board::dho::hw::trim::TRIM_HOLD);
        None
    });
    assert!(trimmed.events.contains(&Event::Trim(1)));
    assert_newest(&trimmed, false, 6, 1);
}

#[test]
fn dvm_start_drops_slot_zero_once() {
    let case = Case { signal: Signal::Both, ..Case::new(2) };
    let [_, _, started, _] = check(&case, false, |source, _| {
        lock(&source.out).dvm_want = Some(crate::board::DvmWant { ch: 0, lane: None });
        None
    });
    let (reg, value) = oos_hal::dho::dvm::start(0, oos_hal::dho::dvm::EXP)[0];
    assert!(started.events.contains(&Event::Write(reg, value)));
    assert_newest(&started, false, 6, 1);
}

#[test]
fn counter_start_drops_slot_zero_once() {
    let case = Case { signal: Signal::Both, ..Case::new(2) };
    for deposit in [false, true] {
        let [_, _, started, _] = check(&case, deposit, |source, io| {
            let want = crate::board::CounterWant { ch: 0, mode: 0, digits: 3, clear: 0 };
            assert!(source.service(io, Some(want)).is_some());
            None
        });
        assert!(started.events.iter().any(|event| matches!(event, Event::Write(oos_hal::dho::counter::CTRL, _))));
        assert_newest(&started, deposit, 6, 1);
    }
}

#[test]
fn counter_latch_drops_slot_zero_only_on_the_due_pass() {
    let case = Case { signal: Signal::Both, ..Case::new(2) };
    let want = crate::board::CounterWant { ch: 0, mode: 0, digits: 3, clear: 0 };
    let outcomes = twice(|| {
        fixture::series(&case, false, "", |source, io, trace| {
            std::array::from_fn::<_, 5, _>(|pass| {
                if pass == 3 {
                    io.clock.advance(Duration::from_millis(350));
                }
                assert!(source.service(io, Some(want)).is_some());
                fixture::dispatch(&case, source, io);
                fixture::observed(source, io, trace, false)
            })
        })
    });
    let slots = outcomes.each_ref().map(|outcome| selections(outcome, false));
    assert_eq!(slots, [vec![1], vec![1], vec![0, 1], vec![1], vec![0, 1]]);
    for (i, outcome) in outcomes.iter().enumerate() {
        assert_eq!(outcome.events.contains(&Event::Write(oos_hal::dho::counter::LATCH, 1)), i == 3);
        assert_newest(outcome, false, 2 * (i as u64 + 1), 1);
    }
}

#[test]
fn dvm_latch_drops_slot_zero_once_without_disarm() {
    let case = Case { signal: Signal::Both, ..Case::new(2) };
    let outcomes = twice(|| {
        fixture::series(&case, false, "", |source, io, trace| {
            lock(&source.out).dvm_want = Some(crate::board::DvmWant { ch: 0, lane: None });
            std::array::from_fn::<_, 4, _>(|pass| {
                if pass == 2 {
                    io.clock.advance(Duration::from_millis(500));
                }
                fixture::dispatch(&case, source, io);
                fixture::observed(source, io, trace, false)
            })
        })
    });
    let slots = outcomes.each_ref().map(|outcome| selections(outcome, false));
    assert_eq!(slots, [vec![1], vec![0, 1], vec![1], vec![0, 1]]);
    assert!(outcomes[2].events.contains(&Event::Write(oos_hal::dho::dvm::LATCH, 1)));
    assert_newest(&outcomes[2], false, 6, 1);
}

#[test]
fn remaining_acquisition_settings_each_drop_slot_zero_once() {
    use crate::model::{Coupling, Slope, BWS, BW_20M};
    let edits: [Edit; 7] = [
        |m, s| {
            m.ch[0].offset = 0.25;
            s.ch[0].offset = 0.25;
        },
        |m, s| {
            m.ch[0].probe = 10.0;
            s.ch[0].probe = 10.0;
        },
        |m, s| {
            m.ch[0].invert = true;
            s.ch[0].invert = true;
        },
        |m, s| {
            m.ch[0].coupling = Coupling::Ac;
            s.ch[0].couple = seq::Couple::Ac;
        },
        |m, s| {
            m.ch[0].bw = BW_20M;
            s.ch[0].bw = BWS[BW_20M].1;
        },
        |m, s| {
            m.holdoff = 1e-3;
            s.holdoff = 1e-3;
        },
        |m, s| {
            m.slope = Slope::Fall;
            s.slope = tpu::Slope::Falling;
        },
    ];
    for edit in edits {
        settings_edit(1, edit);
    }
}
