// SPDX-License-Identifier: GPL-2.0-or-later
//! Acquisition clock: real waits on the unit, explicitly advanced time in host fixtures.
// Preserve the public paths used by the acquisition thread and its fixtures.
pub use super::execute::{
    acquire_batch_observed, acquire_la_with_clock, dma_timeout, execute_bus_with_clock, read_raw_again_with_clock,
    BatchPhase, RawReadIo, DMA_TIMEOUT,
};
pub use super::wait::BatchWait;
use std::time::{Duration, Instant};

pub trait Clock {
    fn now(&self) -> Instant;
    fn wait(&self, duration: Duration);
}
#[derive(Debug)]
pub struct Real;
impl Clock for Real {
    fn now(&self) -> Instant {
        Instant::now()
    }
    fn wait(&self, duration: Duration) {
        std::thread::sleep(duration);
    }
}
