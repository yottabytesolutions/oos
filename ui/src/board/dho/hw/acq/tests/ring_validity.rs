// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::{board::DvmWant, model::Model};
use oos_core::change::{Request as ChangeRequest, Shadow, UiAcq};
use oos_hal::dho::{afg, dvm};
use std::{cell::RefCell, rc::Rc, sync::Arc};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Cancel {
    Calibration,
    Generator { on_hold: bool },
    DvmStart,
    DvmStop,
    Trim,
    Levels,
    Identity,
}

const DVM: DvmWant = DvmWant { ch: 0, lane: None };
const GENERATOR: crate::board::Gen = (true, 0.001, 1.0, 0.0, afg::Wave::Sine, afg::Mod::None);

#[test]
fn calibration_disarms_a_filled_successor_before_reconfiguring() {
    check(Cancel::Calibration, false);
}

#[test]
fn generator_change_on_hold_entry_discards_the_ring() {
    check(Cancel::Generator { on_hold: true }, false);
}

#[test]
fn generator_change_before_hold_discards_the_ring() {
    check(Cancel::Generator { on_hold: false }, false);
}

#[test]
fn generator_change_on_hold_entry_prevents_the_raw_reread() {
    check(Cancel::Generator { on_hold: true }, true);
}

#[test]
fn dvm_start_discards_the_ring_before_hold() {
    check(Cancel::DvmStart, false);
}

#[test]
fn dvm_stop_discards_the_ring_before_hold() {
    check(Cancel::DvmStop, false);
}

#[test]
fn core_trim_discards_the_ring_before_hold() {
    check(Cancel::Trim, false);
}

#[test]
fn level_slots_discard_the_ring_before_hold() {
    check(Cancel::Levels, false);
}

#[test]
fn stop_during_a_run_identity_change_keeps_the_published_capture() {
    check(Cancel::Identity, false);
}

fn check(cancel: Cancel, raw: bool) {
    let (outcome, retained, replay) = twice(|| run_case(cancel, raw));
    let trace = &outcome.events;
    let park = trace.iter().position(|event| matches!(event, Event::Park(_))).unwrap();
    let stop = trace.iter().position(|event| *event == Event::Stop).unwrap();
    let disarm = trace[park..].iter().position(|event| matches!(event, Event::Write(0x4004, v) if v >> 31 != 0));
    let disarm = park + disarm.unwrap();
    let dispatched = trace.iter().rposition(|event| cancel.dispatched(event)).unwrap();
    assert!(disarm <= dispatched);
    assert_eq!(
        stop < disarm,
        matches!(cancel, Cancel::Calibration | Cancel::Generator { on_hold: true } | Cancel::Identity)
    );
    assert_eq!(trace[..park].iter().filter(|event| is_arm(event)).count(), 2);
    assert!(!trace[park..].iter().any(is_arm));
    assert!(!trace[park..].iter().any(|event| matches!(event, Event::Dma(_))), "{cancel:?} reread the ring");
    assert!(retained, "{cancel:?} replaced the published frame");
    assert!(replay.is_none(), "{cancel:?} published a stopped-view reply");
    assert_eq!(outcome.final_ack, 7);
}

impl Cancel {
    fn dispatched(self, event: &Event) -> bool {
        match self {
            Self::Calibration => matches!(event, Event::Calibration(_)),
            Self::Generator { .. } => *event == Event::Generator(Some(GENERATOR)),
            Self::DvmStart => *event == Event::Write(dvm::CFG, dvm::EXP << 1),
            Self::DvmStop => *event == Event::Write(dvm::CFG, 0),
            Self::Trim => *event == Event::Trim(1),
            Self::Levels => matches!(event, Event::Write(0x1040, value) if value >> 31 != 0),
            Self::Identity => matches!(event, Event::Write(0x4004, value) if value >> 31 != 0),
        }
    }
}

fn run_case(cancel: Cancel, raw: bool) -> (Outcome, bool, Option<Frame>) {
    let _scope = Scope::new();
    let want = if raw {
        Settings { tdiv: 1e-3, depth: 1_400_000, ..settings() }
    } else {
        Settings { tdiv: 10e-6, depth: 14_000, ..settings() }
    };
    let model = Model::default();
    let config = crate::model::config();
    let mut shadow = Shadow::new(&model, UiAcq::default(), &config).unwrap();
    let out = requested(&want, shadow.identity(), false);
    lock(&out).dvm_want = (cancel == Cancel::DvmStop).then_some(DVM);
    let (mut io, trace) = sim::fixture(&want, out.clone(), (Run::Selection, 4, false), None);
    if cancel == Cancel::Trim {
        io.cal_adc = adc::CalAdc::from_payload(vec![0; 0x738]);
    }
    let options = Spawn { mask: want.mask, program_afe: false, cal_dir: String::new(), start_up: None };
    let conf = if cancel == Cancel::Trim { "acq.prearm = on\ncal.core_offset = tight" } else { "acq.prearm = on" };
    let switches = Switches::parse(&|_| None, conf, "acq.frame0 = keep", Default::default());
    let started = io.clock.now();
    let mut source = AcqLoop::new(options, switches, out.clone(), Service::default(), None, started, FixedPacing);
    let changed = shadow.observe(&model, UiAcq::default(), &[ChangeRequest::Clear], &config).unwrap().identity();
    let published = Rc::new(RefCell::new(None));
    drive_cancel(&mut io.clock, &out, (cancel, raw), changed, published.clone());
    source.run(&mut io);
    let g = lock(&out);
    let outcome = Outcome::snapshot(&mut trace.borrow_mut(), &g, io.clock.now().duration_since(started));
    let published = published.take().unwrap();
    if raw {
        assert_eq!(published.seq, 4);
        assert!(crate::deepmeas::deep_pairs(&published));
    } else {
        assert_eq!(Frame::read(&published), Frame::stamp(4, 3));
    }
    (outcome, Arc::ptr_eq(&published, &g.frame), g.replay.as_ref().map(Frame::read))
}

fn drive_cancel(
    clock: &mut sim::Virtual,
    out: &HwShared,
    (cancel, raw): (Cancel, bool),
    changed: Identity,
    published: Rc<RefCell<Option<Arc<crate::board::Frame>>>>,
) {
    let shared = out.clone();
    let mut parked = false;
    clock.drive(move |clock, pause| match pause {
        Pause::Park if !parked => {
            parked = true;
            clock.trigger_poll();
            if matches!(cancel, Cancel::Calibration | Cancel::Generator { on_hold: true }) {
                request_stop(clock, &shared, raw);
            }
            let mut g = lock(&shared);
            published.replace(Some(g.frame.clone()));
            match cancel {
                Cancel::Calibration => g.cal_req = Some((vec![0], true, false)),
                Cancel::Generator { .. } => g.afg[0] = Some(GENERATOR),
                Cancel::DvmStart => g.dvm_want = Some(DVM),
                Cancel::DvmStop => g.dvm_want = None,
                Cancel::Trim => clock.advance(super::super::super::trim::TRIM_HOLD),
                Cancel::Levels => g.slots_want = Some(1),
                Cancel::Identity => {
                    g.acquisition = crate::board::identity::Publication::new(changed, false);
                    clock.suppress_stopped_status();
                }
            }
        }
        Pause::Park => request_stop(clock, &shared, raw),
        Pause::Wait(delay) if cancel == Cancel::Identity && parked && delay == Duration::from_micros(500) => {
            if !lock(&shared).hold {
                request_stop(clock, &shared, raw);
            }
        }
        Pause::Wait(delay) if delay == HELD_POLL || delay == Duration::from_millis(20) => {
            lock(&shared).record_flow.shutdown = true;
        }
        Pause::Wait(_) => {}
    });
}

fn request_stop(clock: &sim::Virtual, shared: &HwShared, raw: bool) {
    clock.stop();
    let mut g = lock(shared);
    if raw {
        g.deep_raw = Some(crate::deepmeas::Want { every: false });
    } else {
        g.stop_view = Some((1e-6, 0.0));
    }
}
