// SPDX-License-Identifier: GPL-2.0-or-later
//! Typed observations of the batch acquisition sequence.

use crate::drawpipe::admission::Grant;
use oos_hal::dho::seq::time::{BatchPhase, BatchWait, Clock};
use std::time::{Duration, Instant};

pub(super) struct SourceTiming {
    armed_at: Option<Instant>,
    arm: Duration,
    first_trigger: Duration,
    fill: Duration,
    read: Duration,
    readout: Duration,
    bytes: usize,
    first_trigger_at: Option<Instant>,
    grant: Grant,
}

impl SourceTiming {
    pub(super) fn new(arm: Option<(Instant, Duration)>, grant: Grant) -> Self {
        Self {
            armed_at: arm.map(|(at, _)| at),
            arm: arm.map_or(Duration::ZERO, |(_, duration)| duration),
            first_trigger: Duration::ZERO,
            fill: Duration::ZERO,
            read: Duration::ZERO,
            readout: Duration::ZERO,
            bytes: 0,
            first_trigger_at: None,
            grant,
        }
    }

    pub(super) fn arm<T>(&mut self, clock: &impl Clock, arm: impl FnOnce() -> T) -> T {
        let mut timing = None;
        let result = timed_arm(&mut timing, clock, arm);
        if let Some((at, duration)) = timing {
            self.armed_at = Some(at);
            self.arm = duration;
        }
        result
    }

    pub(super) fn wait(&self, fill_ms: u64) -> BatchWait {
        BatchWait::new(fill_ms, self.armed_at)
    }

    pub(super) fn observe(&mut self, phase: BatchPhase) {
        match phase {
            BatchPhase::Arm(duration) => self.arm = duration,
            BatchPhase::FirstTrigger(duration) => self.first_trigger = duration,
            BatchPhase::FillStarted(at) => self.first_trigger_at = Some(at),
            BatchPhase::Fill(duration) => self.fill = duration,
            BatchPhase::Readout(duration) => self.readout = duration,
            BatchPhase::Read(duration, bytes) => {
                self.read = duration;
                self.bytes = bytes;
            }
        }
    }

    pub(super) fn read_ms(&self) -> f32 {
        (self.read + self.readout).as_secs_f32() * 1e3
    }

    pub(super) fn trigger_wait(&self) -> Duration {
        self.first_trigger + self.fill
    }

    pub(super) fn pre_ms(&self, started: Instant) -> f32 {
        self.first_trigger_at.map_or(0.0, |at| at.saturating_duration_since(started).as_secs_f32() * 1e3)
    }

    pub(super) fn complete(
        &self,
        identity: oos_core::change::Identity,
        read_at: Instant,
        records: u32,
    ) -> crate::drawpipe::clock::StageClock {
        crate::drawpipe::counters().metrics.source_batch(
            identity,
            crate::drawpipe::metrics::SourceBatch {
                arm: self.arm,
                fill: self.fill,
                read: self.read,
                readout: self.readout,
                read_at,
                records,
                serial: self.grant.serial(),
                first_trigger: self.first_trigger,
                grant: self.grant.records(),
                bytes: self.bytes,
            },
        );
        // Admission waits happen after the first complete trigger and feed source.v1 wait_admission.
        // Keep the trigger timestamp here so trigger_composed includes that wait.
        crate::drawpipe::clock::StageClock::read(self.first_trigger_at, read_at)
    }
}

pub(super) fn timed_arm<T>(timing: &mut Option<(Instant, Duration)>, clock: &impl Clock, arm: impl FnOnce() -> T) -> T {
    let started = clock.now();
    let result = arm();
    let at = clock.now();
    *timing = Some((at, at - started));
    result
}
