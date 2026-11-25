// SPDX-License-Identifier: GPL-2.0-or-later

use super::{lock, DoneRead, Event, Pause, Virtual};
use oos_hal::dho::{bus::Bus, seg::sim};
use std::time::Duration;

impl Virtual {
    pub(in super::super) fn drive(&mut self, advance: impl FnMut(&Self, Pause) + 'static) {
        *self.driver.get_mut() = Some(Box::new(advance));
    }

    pub(in super::super) fn mark(&self, event: Event) {
        self.trace.borrow_mut().events.push(event);
    }

    pub(in super::super) fn poke(&self, reg: u32, value: u32) {
        self.engine.borrow_mut().regs.insert(reg, value);
    }

    pub(in super::super) fn suppress_stopped_status(&self) {
        self.signals.hide_stopped.set(true);
    }

    pub(in super::super) fn restore_stopped_status(&self) {
        self.signals.hide_stopped.set(false);
    }

    pub(in super::super) fn fail_done_after_stop(&self) {
        self.signals.done.set(DoneRead::AllOnes);
    }

    pub(in super::super) fn stop_clears_done(&self) {
        self.signals.done.set(DoneRead::Cleared);
    }

    pub(in super::super) fn delay_stopped_status(&self, delay: Duration) {
        self.signals.delay.set(delay);
    }

    pub(in super::super) fn error_reading_done_after_stop(&self) {
        self.signals.done.set(DoneRead::Error);
    }

    pub(in super::super) fn capture_armed(&self) -> bool {
        self.engine.borrow().capture_armed()
    }

    pub(in super::super) fn record_triggers(&self, count: u32) {
        assert!(self.capture_armed());
        self.engine.borrow_mut().record_triggers(count);
    }

    pub(in super::super) fn advance(&self, duration: Duration) {
        self.elapse(duration, Event::Wait(duration));
    }

    pub(in super::super) fn trigger_poll(&self) {
        let mut bus = sim::SimBus(self.engine.clone());
        // The simulator records the next four triggers on a DONE poll.
        assert_ne!(bus.rd(0x4034) & oos_hal::dho::scu::status::WAIT_TRIG, 0);
        assert_ne!(bus.rd(0x4080), 0);
        assert_eq!(bus.rd(0x4010), 3);
    }

    pub(in super::super) fn stop_triggers(&self) {
        self.engine.borrow_mut().per_poll = 0;
    }

    pub(in super::super) fn resume_triggers(&self, count: u32) {
        self.engine.borrow_mut().per_poll = count;
    }

    pub(in super::super) fn capture_mask(&self, mask: u32) {
        self.engine.borrow_mut().ratio = oos_hal::dho::hz::ratio(mask);
    }

    pub(in super::super) fn release(&self) {
        let event = lock(&self.out).record_flow.source.clone();
        let seen = event.generation();
        self.hold(false);
        assert_ne!(event.generation(), seen);
        self.mark(Event::SourceWake);
    }
}
