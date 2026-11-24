// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::board::{dho::replay::Batch, identity::Publication, Frame, HwShared};
use oos_hal::dho::{
    bus::{Bus, DmaBuf, Recorder},
    hz, scu,
    seq::time::{BatchWait, Clock},
};
use oos_hal::sync::lock;
use std::collections::BTreeMap;

struct Script<'a> {
    start: Instant,
    elapsed: Cell<Duration>,
    out: &'a HwShared,
    hold_at: Option<Duration>,
    calibrate_at: Option<Duration>,
}

impl<'a> Script<'a> {
    fn new(out: &'a HwShared, hold_at: Option<Duration>, calibrate_at: Option<Duration>) -> Self {
        Self { start: Instant::now(), elapsed: Cell::default(), out, hold_at, calibrate_at }
    }
}

impl Clock for Script<'_> {
    fn now(&self) -> Instant {
        self.start + self.elapsed.get()
    }

    fn wait(&self, duration: Duration) {
        let elapsed = self.elapsed.get() + duration;
        self.elapsed.set(elapsed);
        let mut g = lock(self.out);
        if self.hold_at.is_some_and(|at| elapsed >= at) {
            g.hold = true;
            g.final_req = 1;
        }
        if self.calibrate_at.is_some_and(|at| elapsed >= at) {
            g.cal_req = Some((vec![0], false, false));
        }
    }
}

struct Ring<'a> {
    clock: &'a Script<'a>,
    done_at: Duration,
    bus: Recorder,
    slots: BTreeMap<u32, u8>,
    armed: Option<u32>,
    writes_before_poll: Option<usize>,
    played: &'a Cell<u8>,
}

impl Bus for Ring<'_> {
    fn wr(&mut self, reg: u32, value: u32) {
        self.bus.wr(reg, value);
        self.bus.regs.insert(reg, value);
        if reg != 0x4004 {
            return;
        }
        if value >> 31 != 0 {
            self.armed = None;
        }
        if value & 1 != 0 {
            let first = self.bus.rd(0x4014);
            match self.bus.rd(0x400c) & 0x6200_0000 {
                0x2200_0000 => {
                    self.armed = Some(first);
                    self.slots.insert(first, 0); // The pre-trigger write destroys this slot immediately.
                }
                0x6000_0000 => self.played.set(self.slots[&first]),
                _ => {}
            }
        }
    }

    fn rd(&mut self, reg: u32) -> u32 {
        match reg {
            0x4034 => scu::status::STOPPED,
            0x4080 => {
                self.writes_before_poll.get_or_insert(self.bus.writes.len());
                if let Some(first) = self.armed.filter(|_| self.clock.elapsed.get() >= self.done_at) {
                    self.slots.insert(first, 29);
                    scu::PLAYINFO_DONE
                } else {
                    0
                }
            }
            _ => self.bus.rd(reg),
        }
    }
}

struct Codes<'a>(&'a Cell<u8>);

impl std::io::Read for Codes<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        bytes.fill(self.0.get());
        Ok(bytes.len())
    }
}

fn ms(value: u64) -> Duration {
    Duration::from_millis(value)
}

fn run(
    script: &Script<'_>,
    serial: bool,
    done_at: Duration,
    check: impl FnOnce(&mut Ring<'_>, &mut seq::State, Option<Batch>, Result<u32, String>, bool),
) {
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let settings = Settings { auto: false, depth: 4000, compress: 4, ..Settings::default() };
    let params = hz::prep(1, 400_000_000_000, 4000, 0);
    let mut state = seq::State::handover(1, None);
    let previous = Batch::new(settings.clone(), params, (0, false), Default::default(), None);
    let plan = ArmPlan::new(&settings, Default::default(), false, None, serial);
    let played = Cell::new(0);
    let mut ring = Ring {
        clock: script,
        done_at,
        bus: Recorder::default(),
        slots: [(0, 17)].into(),
        armed: None,
        writes_before_poll: None,
        played: &played,
    };
    let mut dma = DmaBuf::new();
    let asked = (None, Slow::Off);
    let wait = FirstWait::default();
    let result = seq::time::acquire_batch_observed(
        &mut state,
        &settings,
        &params,
        &mut ring,
        &mut Codes(&played),
        &mut dma,
        false,
        (BatchWait::new(20, None), script),
        |_| {},
        || wait.canceled(&lock(script.out), &plan, asked, (settings.auto, false), script.now()),
        (|| false, |_| !read_obsolete(&lock(script.out), &plan, asked)),
        |_| {},
    );
    let mut previous = Some(previous);
    let fresh = match &result {
        Ok(count) => {
            assert_eq!(dma.data().first(), Some(&29));
            assert!(dma.data().iter().all(|&byte| byte == 29));
            previous = Some(Batch::new(settings, params, (count - 1, false), Default::default(), None));
            true
        }
        Err(error) => {
            if needs_disarm(error) {
                crate::board::dho::lifecycle::disarm(|| state.disarm_capture(&mut ring, script));
            }
            previous.take_if(|batch| batch.index < seq::frames(&settings, &params));
            serial && previous.is_some()
        }
    };
    check(&mut ring, &mut state, previous, result, fresh);
}

fn reread(ring: &mut Ring<'_>, state: &mut seq::State, batch: Batch, expected: u8) {
    let mut dma = DmaBuf::new();
    let settings = batch.plan(None).unwrap();
    let steps = state.reread_steps(&settings, &batch.params, batch.index);
    seq::execute_bus(&steps, ring, &mut Codes(ring.played), &mut dma, |_| {}).unwrap();
    assert!(!dma.data().is_empty());
    assert!(dma.data().iter().all(|&byte| byte == expected));
}

#[test]
fn serial_stop_reads_a_trigger_that_completes_after_hold() {
    let out = HwShared::default();
    let script = Script::new(&out, Some(ms(1)), None);
    run(&script, true, ms(40), |ring, state, batch, result, fresh| {
        assert_eq!(result, Ok(1));
        let batch = batch.unwrap();
        assert_eq!(batch.index, 0);
        assert!(fresh && lock(&out).hold);
        reread(ring, state, batch, 29);
    });
}

#[test]
fn overlapping_normal_stop_cancels_at_the_next_control_poll() {
    let out = HwShared::default();
    let script = Script::new(&out, Some(ms(1)), None);
    run(&script, false, ms(100_000), |ring, _, _, result, _| {
        assert_eq!(result, Err(seq::NEW_SETTINGS.into()));
        assert!(script.elapsed.get() <= ms(11));
        assert_eq!(ring.armed, None);
    });
}

#[test]
fn overlapping_trigger_timeout_disarms_even_before_hold_is_seen() {
    let out = HwShared::default();
    let script = Script::new(&out, None, None);
    run(&script, false, ms(100_000), |ring, _, _, result, _| {
        assert!(is_stall(&result.unwrap_err()));
        assert_eq!(ring.armed, None);
        let text = crate::drawpipe::counters().acquisition.text();
        assert!(text.contains("disarm=1"), "{text}");
    });
}

#[test]
fn calibration_waits_for_first_record_and_does_not_disarm_it() {
    let out = HwShared::default();
    let script = Script::new(&out, None, Some(ms(1)));
    run(&script, false, ms(40), |_, _, _, result, _| {
        assert_eq!(result, Ok(1));
        assert_eq!(script.elapsed.get(), ms(40));
        assert!(lock(&out).cal_req.is_some());
        let text = crate::drawpipe::counters().acquisition.text();
        assert!(text.contains("disarm=0"), "{text}");
    });
}

// Expected writes for a one-frame arm after State::handover(1, None).
const ZERO_ARM: [(u32, u32); 29] = [
    (0x4004, 0),
    (0x4004, 0),
    (0x4004, 0),
    (0x4004, 0),
    (0x4004, 0x8000_0000),
    (0x4004, 0),
    (0x4080, 0x4000_0010),
    (0x4080, 0xc000_0010),
    (0x4080, 0x4000_0010),
    (0x1000, 1),
    (0x1000, 0),
    (0x1000, 0),
    (0x1000, 0x10),
    (0x1000, 0),
    (0x4004, 0),
    (0x4004, 0),
    (0x4004, 0),
    (0x4008, 0x0080_0001),
    (0x4008, 0x0080_0001),
    (0x406c, 0),
    (0x4068, 0),
    (0x400c, 0x3000_0000),
    (0x4010, 0),
    (0x4014, 0),
    (0x4018, 0),
    (0x400c, 0x2000_0000),
    (0x400c, 0x2200_0000),
    (0x4008, 0x0080_0001),
    (0x4004, 1),
];

#[test]
fn default_serial_and_overlapping_arms_match_the_recorded_writes() {
    for serial in [true, false] {
        let out = HwShared::default();
        let script = Script::new(&out, None, None);
        run(&script, serial, ms(40), |ring, _, batch, result, _| {
            assert_eq!(result, Ok(1));
            assert_eq!(batch.unwrap().index, 0);
            assert_eq!(&ring.bus.writes[..ring.writes_before_poll.unwrap()], &ZERO_ARM, "serial={serial}");
        });
    }
}

#[test]
fn default_serial_stop_invalidates_the_overwritten_slot_on_timeout() {
    let out = HwShared::default();
    let script = Script::new(&out, Some(ms(1)), None);
    run(&script, true, ms(100_000), |ring, _, batch, result, fresh| {
        assert!(is_stall(&result.unwrap_err()));
        assert_eq!(ring.armed, None);
        assert_eq!(ring.slots[&0], 0);
        assert!(batch.is_none() && !fresh);
    });
}

#[test]
fn serial_record_admission_yields_to_calibration_force_epoch_and_route() {
    let out = HwShared::default();
    let script = Script::new(&out, None, None);
    lock(&out).queue.push_back(Frame::default().into());
    let settings = Settings::default();
    let mut shadow =
        oos_core::change::Shadow::new(&crate::model::Model::default(), Default::default(), &crate::model::config())
            .unwrap();
    let id = shadow.identity();
    let epoch = shadow
        .observe(
            &crate::model::Model::default(),
            Default::default(),
            &[oos_core::change::Request::Clear],
            &crate::model::config(),
        )
        .unwrap()
        .identity();
    lock(&out).acquisition = Publication::new(id, false);
    let plan = ArmPlan::new(&settings, id, false, None, true);
    let asked = (None, Slow::Off);
    assert!(!admission_interrupted(&lock(&out), &plan, asked));
    for step in 0..4 {
        let mut wait = crate::board::dho::publication::RecordWait::default();
        assert!(wait.poll(
            &out,
            None,
            &oos_hal::dho::seq::time::Real,
            || {
                script.wait(ms(1));
                let mut g = lock(&out);
                g.cal_req = (step == 0).then(|| (vec![0], true, false));
                g.force = step == 1;
                g.acquisition = Publication::new(if step == 2 { epoch } else { id }, step == 3);
                assert!(admission_interrupted(&g, &plan, asked), "control {step}");
                true
            },
            |_| {}
        ));
        assert!(
            !plan.read_allowed(&oos_hal::dho::seq::time::Real, || true, || read_obsolete(&lock(&out), &plan, asked)),
            "control {step}"
        );
    }
}

#[test]
fn stop_keeps_an_overlapping_record_batch_on_the_regular_publication_path() {
    let settings = Settings { frames: 4, ..Settings::default() };
    let out = HwShared::default();
    let asked = (None, Slow::Off);
    let plan = ArmPlan::new(&settings, Default::default(), false, None, false);
    let serial = ArmPlan::new(&settings, Default::default(), false, None, true);
    let script = Script::new(&out, Some(ms(10)), None);
    assert_eq!(held_request(&lock(&out), &plan, asked, 4, false), None);
    script.wait(ms(10));
    assert_eq!(held_request(&lock(&out), &plan, asked, 4, false), None);
    assert_eq!(held_request(&lock(&out), &serial, asked, 4, false), None, "a retained overlapping arm");
    assert_eq!(held_request(&lock(&out), &serial, asked, 1, false), Some(1));
}
