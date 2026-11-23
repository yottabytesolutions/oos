// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! Trigger polling and ring fill, using the acquisition clock.

use super::{time::Clock, Step};
use crate::dho::{hz, scu};
use std::time::{Duration, Instant};

/// What `execute_until` returns when `stop` ended a poll: the same state of the FPGA as a poll that timed out.
pub const NEW_SETTINGS: &str = "stopped for new settings";

/// The wait of a batch after its arm: done (= the first frame is complete), then the ring registers for the log.
pub fn wait_steps(p: &hz::Params) -> Vec<Step> {
    // at least the record itself, twice over: slow time bases with the rate halved by the LA take seconds
    let record_ms = p.memsel as u64 * 1000 / p.rate.max(1);
    vec![
        Step::Poll {
            reg: 0x4080,
            bit: scu::PLAYINFO_DONE,
            ms: 2000.max(2 * record_ms + 500),
            what: "acquisition done",
        },
        Step::Peek(&[0x4034, 0x4010, 0x4014]),
    ]
}

pub(super) const STOP_POLL: Duration = Duration::from_millis(10);

/// Fill budget and the end of arm writes when the caller armed ahead.
#[derive(Clone, Copy, Debug)]
pub struct BatchWait {
    fill: Duration,
    armed_at: Option<Instant>,
    pub(super) first: u32,
}

impl BatchWait {
    pub fn new(fill_ms: u64, armed_at: Option<Instant>) -> Self {
        Self { fill: Duration::from_millis(fill_ms), armed_at, first: 0 }
    }

    /// Apply the same ring start to arm, fill count and playback.
    pub fn from_frame(self, first: u32) -> Self {
        Self { first, ..self }
    }

    pub(super) fn fill(self) -> Duration {
        self.fill
    }

    pub(super) fn armed_at(self) -> Option<Instant> {
        self.armed_at
    }
}

pub(super) fn record_time(points: u32, rate: u64) -> Duration {
    Duration::from_nanos(u64::from(points) * 1_000_000_000 / rate.max(1))
}

pub(super) fn fill_count(
    bar: &mut impl crate::dho::bus::Bus,
    (nf, p): (u32, &hz::Params),
    (wait, started, estimate): (BatchWait, Instant, bool),
    clock: &impl Clock,
    (idle, stop): (&mut impl FnMut() -> bool, &impl Fn() -> bool),
) -> u32 {
    let record = if estimate { record_time(p.memsel, p.rate) } else { Duration::ZERO };
    let step = Duration::from_micros(200);
    let mut next_stop = started + STOP_POLL;
    loop {
        let count = bar.rd(0x4010).wrapping_add(1).saturating_sub(wait.first).clamp(1, nf);
        let now = clock.now();
        let left = wait.fill().saturating_sub(now.saturating_duration_since(started));
        if count == nf || left.is_zero() {
            return count;
        }
        if now >= next_stop {
            if stop() {
                return count;
            }
            next_stop = now + STOP_POLL;
        }
        if !idle() {
            let remaining = record.saturating_mul(nf - count);
            let delay = remaining.clamp(step, STOP_POLL);
            clock.wait(delay.min(left).min(next_stop.saturating_duration_since(now)));
        }
    }
}

pub(super) fn poll<B: crate::dho::bus::Bus>(
    step: &Step,
    bar: &mut B,
    log: &mut impl FnMut(String),
    stop: &impl Fn() -> bool,
    clock: &impl Clock,
) -> Result<(), String> {
    poll_after(step, bar, log, stop, clock, Duration::ZERO)
}

pub(super) fn poll_after(
    step: &Step,
    bar: &mut impl crate::dho::bus::Bus,
    log: &mut impl FnMut(String),
    stop: &impl Fn() -> bool,
    clock: &impl Clock,
    delay: Duration,
) -> Result<(), String> {
    let Step::Poll { reg, bit, ms, what } = step else { return Ok(()) };
    let t = clock.now();
    let mut last = bar.rd(*reg);
    let delay = delay.min(STOP_POLL);
    if last & bit == 0 && !delay.is_zero() {
        if stop() {
            return Err(NEW_SETTINGS.into());
        }
        clock.wait(delay);
        last = bar.rd(*reg);
        if last & bit == 0 && stop() {
            return Err(NEW_SETTINGS.into());
        }
    }
    let mut k = 0u32;
    while last & bit == 0 && clock.now().saturating_duration_since(t).as_millis() < *ms as u128 {
        clock.wait(std::time::Duration::from_micros(500));
        last = bar.rd(*reg);
        k += 1;
        if k.is_multiple_of(20) && last & bit == 0 && stop() {
            log(format!(
                "{} {} ({:.1} ms)",
                what,
                NEW_SETTINGS,
                clock.now().saturating_duration_since(t).as_secs_f64() * 1e3
            ));
            return Err(NEW_SETTINGS.into());
        }
    }
    let ok = last & bit != 0;
    log(format!(
        "{} {} ({:#x} = {:#010x}, {:.1} ms)",
        what,
        if ok { "ok" } else { "TIMEOUT" },
        reg,
        last,
        clock.now().saturating_duration_since(t).as_secs_f64() * 1e3
    ));
    if !ok && *reg == 0x4080 {
        // 0x4030 bit 20 = mask/zone read-back started, [3:0] = FSM state (5 = waiting for the column window)
        let st = bar.rd(0x4030);
        return Err(format!(
            "no acquisition (0x4080 bit30 not set; 0x4030 = {:#010x}, readback {}, state {})",
            st,
            st >> 20 & 1,
            st & 0xf
        ));
    }

    Ok(())
}
