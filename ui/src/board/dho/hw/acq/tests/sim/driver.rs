// SPDX-License-Identifier: GPL-2.0-or-later

use super::{lock, Event, Pause, Virtual};
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
        self.hide_stopped.set(true);
    }

    pub(in super::super) fn advance(&self, duration: Duration) {
        self.elapse(duration, Event::Wait(duration));
    }

    pub(in super::super) fn release(&self) {
        let event = lock(&self.out).record_flow.source.clone();
        let seen = event.generation();
        self.hold(false);
        assert_ne!(event.generation(), seen);
        self.mark(Event::SourceWake);
    }
}
