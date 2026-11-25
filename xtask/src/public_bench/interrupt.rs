// SPDX-License-Identifier: GPL-2.0-or-later

use std::time::{Duration, Instant};

use oos_hal::system::TerminationSignals;

use super::{Clock, Error};
use crate::unit_tools::scpi::{Scpi, TimedReply};

#[derive(Debug)]
pub(super) struct HostClock {
    started: Instant,
    signals: TerminationSignals,
}

impl HostClock {
    pub(super) fn start() -> Result<Self, Error> {
        let signals = TerminationSignals::listen_until_exit().map_err(Error::InterruptControl)?;
        Ok(Self { started: Instant::now(), signals })
    }
}

impl Clock for HostClock {
    fn now(&self) -> Duration {
        self.started.elapsed()
    }

    fn check(&self) -> Result<(), Error> {
        self.wait(Duration::ZERO)
    }

    fn wait(&self, duration: Duration) -> Result<(), Error> {
        match self.signals.wait(duration).map_err(Error::InterruptControl)? {
            Some(code) => Err(Error::Interrupted(code)),
            None => Ok(()),
        }
    }
}

/// Leave each started exchange intact. Check before the next one and let
/// restoration use the original connection with cancellation bypassed.
#[derive(Debug)]
pub(super) struct InterruptibleScpi<'a, S, C> {
    scope: &'a mut S,
    clock: &'a C,
}

impl<'a, S: Scpi, C: Clock> InterruptibleScpi<'a, S, C> {
    pub(super) fn new(scope: &'a mut S, clock: &'a C) -> Self {
        Self { scope, clock }
    }

    fn check(&self) -> Result<(), String> {
        self.clock.check().map_err(|error| error.to_string())
    }
}

impl<S: Scpi, C: Clock> Scpi for InterruptibleScpi<'_, S, C> {
    fn send(&mut self, command: &str) -> Result<(), String> {
        self.check()?;
        self.scope.send(command)
    }

    fn query_timed<T>(&mut self, command: &str, now: impl FnMut() -> T) -> Result<TimedReply<T>, String> {
        self.check()?;
        self.scope.query_timed(command, now)
    }

    fn reconnect(&mut self) -> Result<(), String> {
        self.check()?;
        self.scope.reconnect()
    }
}
