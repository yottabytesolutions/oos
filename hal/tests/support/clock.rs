// SPDX-License-Identifier: GPL-2.0-or-later
use oos_hal::dho::seq::time::Clock;
use std::time::{Duration, Instant};
#[derive(Debug)]
pub struct Manual {
    now: std::cell::Cell<Instant>,
}
impl Manual {
    pub fn new(now: Instant) -> Self {
        Self { now: std::cell::Cell::new(now) }
    }
    pub fn advance(&self, duration: Duration) {
        self.now.set(self.now.get() + duration);
    }
}
impl Clock for Manual {
    fn now(&self) -> Instant {
        self.now.get()
    }
    fn wait(&self, duration: Duration) {
        self.advance(duration);
    }
}
