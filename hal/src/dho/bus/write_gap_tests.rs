// SPDX-License-Identifier: GPL-2.0-or-later

use super::{Bus, DmaBuf, Paced, Recorder};
use crate::dho::{boot, hz, scu, seq, Write};
use std::{cell::RefCell, time::Duration};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Event {
    Write(Write),
    Wait(Duration),
    Read(u32),
}

struct Trace<'a> {
    registers: Recorder,
    events: &'a RefCell<Vec<Event>>,
}

impl Bus for Trace<'_> {
    fn wr(&mut self, off: u32, value: u32) {
        self.registers.wr(off, value);
        self.events.borrow_mut().push(Event::Write((off, value)));
    }

    fn rd(&mut self, off: u32) -> u32 {
        self.events.borrow_mut().push(Event::Read(off));
        self.registers.rd(off)
    }
}

const WAIT: Event = Event::Wait(Duration::from_micros(10));

#[test]
fn batch_arm_waits_once_after_the_last_write_before_polling() {
    let settings = seq::Settings::default();
    let params = hz::prep(1, seq::tdiv_fs(settings.tdiv), 0, 0);
    let steps = seq::State::handover(1, None).arm_steps(&settings, &params);
    let events = RefCell::new(Vec::new());
    let mut bus = Trace { registers: Recorder::default(), events: &events };
    bus.registers.regs.insert(0x4080, scu::PLAYINFO_DONE);
    let mut batch = Paced { bus: &mut bus, batch_settle: true, wait: |d| events.borrow_mut().push(Event::Wait(d)) };
    seq::execute_bus(&steps, &mut batch, &mut &[][..], &mut DmaBuf::new(), |_| {}).unwrap();
    let mut expected: Vec<_> = bus.registers.writes.into_iter().map(Event::Write).collect();
    assert_eq!(expected.len(), 29);
    expected.extend([WAIT, Event::Read(0x4080), Event::Read(0x4034), Event::Read(0x4010), Event::Read(0x4014)]);
    assert_eq!(events.into_inner(), expected);
}

#[test]
fn ordinary_boot_writes_keep_their_individual_waits() {
    let events = RefCell::new(Vec::new());
    let mut bus = Trace { registers: Recorder::default(), events: &events };
    let mut ordinary = Paced { bus: &mut bus, batch_settle: false, wait: |d| events.borrow_mut().push(Event::Wait(d)) };
    let writes = boot::regs::scu_init(&mut boot::Shadows::default());
    assert!(writes.len() > 1);
    ordinary.apply(&writes);
    let expected: Vec<_> = writes.into_iter().flat_map(|w| [Event::Write(w), WAIT]).collect();
    assert_eq!(events.into_inner(), expected);
}

#[test]
fn a_status_clear_settles_once_and_an_empty_sequence_does_not_wait() {
    let events = RefCell::new(Vec::new());
    let mut bus = Trace { registers: Recorder::default(), events: &events };
    let mut batch = Paced { bus: &mut bus, batch_settle: true, wait: |d| events.borrow_mut().push(Event::Wait(d)) };
    let writes = scu::clear_status(0x11d2);
    batch.apply(&[]);
    batch.apply(&writes);
    assert_eq!(events.into_inner(), [Event::Write(writes[0]), Event::Write(writes[1]), WAIT]);
}
