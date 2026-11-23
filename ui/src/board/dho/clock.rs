// SPDX-License-Identifier: GPL-2.0-or-later

use crate::drawpipe::{Pipe, SourceWake};
use oos_hal::dho::seq::time::{Clock, Real};
use std::time::Duration;

pub(super) trait Wake {
    fn generation(&self) -> u64;
    fn wait(&self, observed: u64, timeout: Duration) -> Option<Duration>;
}

impl Wake for SourceWake {
    fn generation(&self) -> u64 {
        self.generation()
    }

    fn wait(&self, observed: u64, timeout: Duration) -> Option<Duration> {
        self.wait(observed, timeout)
    }
}

impl Wake for Pipe {
    fn generation(&self) -> u64 {
        self.source_generation()
    }

    fn wait(&self, observed: u64, timeout: Duration) -> Option<Duration> {
        self.wait_source(observed, timeout)
    }
}

pub(super) trait LoopClock: Clock {
    fn park(&self, event: &impl Wake, observed: u64, timeout: Duration) -> Option<Duration>;
}

impl LoopClock for Real {
    fn park(&self, event: &impl Wake, observed: u64, timeout: Duration) -> Option<Duration> {
        event.wait(observed, timeout)
    }
}
