// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! Batch acquisition and execution of register and DMA steps.

use super::arm::batch_arm;
use super::time::{self, Clock};
use super::wait::{fill_count, poll, poll_after, record_time, BatchWait};
use super::{frames, raw_settings, read_len, unpad_in, Settings, State, Step, NEW_SETTINGS};
use crate::dho::hz;
use std::time::{Duration, Instant};

#[cfg(target_os = "linux")]
pub(super) mod linux;
#[cfg(test)]
mod tests;

#[derive(Debug)]
pub enum BatchPhase {
    Arm(Duration),
    FirstTrigger(Duration),
    FillStarted(Instant),
    Fill(Duration),
    Readout(Duration),
    Read(Duration, usize),
}

/// Like `acquire_batch_idle`, with typed timestamps for the acquisition thread.
/// The caller handles disarming after a canceled or timed-out acquisition.
#[allow(clippy::too_many_arguments)] // Mirrors the acquisition boundary and adds its observer.
pub fn acquire_batch_observed<B: crate::dho::bus::Bus, R: std::io::Read>(
    st: &mut State,
    s: &Settings,
    p: &hz::Params,
    bar: &mut B,
    c2h: &mut R,
    dma: &mut crate::dho::bus::DmaBuf,
    armed: bool,
    (wait, clock): (BatchWait, &impl Clock),
    log: impl FnMut(String),
    stop: impl Fn() -> bool,
    (stop_fill, before_read): (impl Fn() -> bool, impl FnMut(u32) -> bool),
    observe: impl FnMut(BatchPhase),
) -> Result<u32, String> {
    acquire_batch_phases(
        st,
        s,
        p,
        bar,
        c2h,
        dma,
        armed,
        wait,
        log,
        stop,
        || false,
        clock,
        Some((observe, stop_fill, before_read)),
    )
}

/// One batch on the unit: arm `frames(s, p)` frames, wait for the first, then let the ring fill until all frames are
/// recorded or `wait_ms` have passed since the first one (0x4010 reads back the last recorded frame, so the count is
/// `0x4010 - first + 1`), then play and read exactly those frames into `dma`.
/// `armed`: `arm_only_steps` already armed this batch with the same `s` and `p`; only the wait is left.
/// Returns the count; the bytes (`count` x `frame_bytes`) are `dma.data()`.
/// Generic over the bus, so the host model (`seg::sim::Engine`) runs it too.
#[allow(clippy::too_many_arguments)] // Retains the existing register, DMA, logging and cancellation boundary.
pub fn acquire_batch<B: crate::dho::bus::Bus, R: std::io::Read>(
    st: &mut State,
    s: &Settings,
    p: &hz::Params,
    bar: &mut B,
    c2h: &mut R,
    dma: &mut crate::dho::bus::DmaBuf,
    armed: bool,
    wait_ms: u64,
    log: impl FnMut(String),
) -> Result<u32, String> {
    acquire_batch_until(st, s, p, bar, c2h, dma, armed, wait_ms, log, || false)
}

/// `acquire_batch` whose wait for a trigger ends with `Err(NEW_SETTINGS)` as soon as `stop()` says so (checked every
/// 10 ms): in NORMAL without a trigger the wait lasts 2 s, and new settings would wait for it.
#[allow(clippy::too_many_arguments)] // Retains the existing register, DMA, logging and cancellation boundary.
pub fn acquire_batch_until<B: crate::dho::bus::Bus, R: std::io::Read>(
    st: &mut State,
    s: &Settings,
    p: &hz::Params,
    bar: &mut B,
    c2h: &mut R,
    dma: &mut crate::dho::bus::DmaBuf,
    armed: bool,
    wait_ms: u64,
    log: impl FnMut(String),
    stop: impl Fn() -> bool,
) -> Result<u32, String> {
    acquire_batch_idle(st, s, p, bar, c2h, dma, armed, wait_ms, log, stop, || false)
}

/// `acquire_batch_until` that lets the caller work while the ring fills: `idle()` is called instead of the sleep
/// between two looks at the frame count, and the count is read after every call, so the read waits at most one call.
/// `idle` returns false when it has nothing to do (the wait then sleeps as without it).
#[allow(clippy::too_many_arguments)] // Retains the existing register, DMA, logging and cancellation boundary.
pub fn acquire_batch_idle<B: crate::dho::bus::Bus, R: std::io::Read>(
    st: &mut State,
    s: &Settings,
    p: &hz::Params,
    bar: &mut B,
    c2h: &mut R,
    dma: &mut crate::dho::bus::DmaBuf,
    armed: bool,
    wait_ms: u64,
    log: impl FnMut(String),
    stop: impl Fn() -> bool,
    idle: impl FnMut() -> bool,
) -> Result<u32, String> {
    acquire_batch_with_clock(st, s, p, bar, c2h, dma, armed, wait_ms, log, stop, idle, &time::Real)
}

/// Same acquisition using an explicit clock, including all arm/read polls.
#[allow(clippy::too_many_arguments)] // Mirrors the existing generic acquisition boundary, plus its clock.
pub fn acquire_batch_with_clock<B: crate::dho::bus::Bus, R: std::io::Read>(
    st: &mut State,
    s: &Settings,
    p: &hz::Params,
    bar: &mut B,
    c2h: &mut R,
    dma: &mut crate::dho::bus::DmaBuf,
    armed: bool,
    wait_ms: u64,
    log: impl FnMut(String),
    stop: impl Fn() -> bool,
    idle: impl FnMut() -> bool,
    clock: &impl Clock,
) -> Result<u32, String> {
    acquire_batch_phases(
        st,
        s,
        p,
        bar,
        c2h,
        dma,
        armed,
        BatchWait::new(wait_ms, None),
        log,
        stop,
        idle,
        clock,
        None::<(fn(BatchPhase), fn() -> bool, fn(u32) -> bool)>,
    )
}

#[allow(clippy::too_many_arguments)] // Shares execution with the existing clock and optional phase observer.
fn acquire_batch_phases<B: crate::dho::bus::Bus, R: std::io::Read>(
    st: &mut State,
    s: &Settings,
    p: &hz::Params,
    bar: &mut B,
    c2h: &mut R,
    dma: &mut crate::dho::bus::DmaBuf,
    armed: bool,
    wait: BatchWait,
    mut log: impl FnMut(String),
    stop: impl Fn() -> bool,
    mut idle: impl FnMut() -> bool,
    clock: &impl Clock,
    mut observed: Option<(impl FnMut(BatchPhase), impl Fn() -> bool, impl FnMut(u32) -> bool)>,
) -> Result<u32, String> {
    let arm = batch_arm(st, s, p, wait.first, armed)?;
    let observed_fill = observed.is_some();
    if let Some((observe, _, _)) = observed.as_mut() {
        first_record(&arm, wait.armed_at(), p, (bar, c2h, dma), (&mut log, &stop, clock), observe)?;
    } else {
        execute_with_clock(&arm, bar, c2h, dma, &mut log, &stop, clock)?;
    }
    let started = clock.now();
    if let Some((observe, _, _)) = observed.as_mut() {
        observe(BatchPhase::FillStarted(started));
    }
    let nf = frames(s, p);
    let stop_fill = || observed.as_ref().is_some_and(|(_, stop_fill, _)| stop_fill());
    let count = fill_count(bar, (nf, p), (wait, started, observed_fill), clock, (&mut idle, &stop_fill));
    let fill = clock.now().saturating_duration_since(started);
    log(format!("{} of {} frames after {:.1} ms", count, nf, fill.as_secs_f64() * 1e3));
    if let Some((observe, _, before_read)) = observed.as_mut() {
        observe(BatchPhase::Fill(fill));
        if !before_read(count) {
            return Err(NEW_SETTINGS.into());
        }
    }
    let steps = st.read_steps_at(s, p, wait.first, count);
    read_batch(&steps, bar, c2h, dma, &mut log, clock, observed.map(|(observe, _, _)| observe))?;
    Ok(count)
}

fn first_record(
    arm: &[Step],
    armed_at: Option<Instant>,
    p: &hz::Params,
    (bar, c2h, dma): (&mut impl crate::dho::bus::Bus, &mut impl std::io::Read, &mut crate::dho::bus::DmaBuf),
    (log, stop, clock): (&mut impl FnMut(String), &impl Fn() -> bool, &impl Clock),
    observe: &mut impl FnMut(BatchPhase),
) -> Result<(), String> {
    let at = arm.iter().position(|step| matches!(step, Step::Poll { .. })).unwrap_or(arm.len());
    let (writes, steps) = arm.split_at(at);
    let started = clock.now();
    execute_with_clock(writes, bar, c2h, dma, log, stop, clock)?;
    let now = clock.now();
    if !writes.is_empty() {
        observe(BatchPhase::Arm(now.saturating_duration_since(started)));
    }
    let armed_at = armed_at.unwrap_or(now);
    if let Some((step, tail)) = steps.split_first() {
        let delay =
            record_time(p.memsel.saturating_sub(p.pre), p.rate).saturating_sub(now.saturating_duration_since(armed_at));
        poll_after(step, bar, log, stop, clock, delay)?;
        execute_with_clock(tail, bar, c2h, dma, log, stop, clock)?;
    }
    observe(BatchPhase::FirstTrigger(clock.now().saturating_duration_since(armed_at)));
    Ok(())
}

fn read_batch<B: crate::dho::bus::Bus, R: std::io::Read>(
    steps: &[Step],
    bar: &mut B,
    c2h: &mut R,
    dma: &mut crate::dho::bus::DmaBuf,
    log: &mut impl FnMut(String),
    clock: &impl Clock,
    observe: Option<impl FnMut(BatchPhase)>,
) -> Result<(), String> {
    let Some(mut observe) = observe else {
        return execute_with_clock(steps, bar, c2h, dma, log, &|| false, clock);
    };
    let start = steps.iter().position(|step| matches!(step, Step::Wr(_))).unwrap_or(steps.len());
    let (before, rest) = steps.split_at(start);
    let writes = rest.iter().take_while(|step| matches!(step, Step::Wr(_))).count();
    let (registers, rest) = rest.split_at(writes);
    execute_with_clock(before, bar, c2h, dma, log, &|| false, clock)?;
    let started = clock.now();
    execute_with_clock(registers, bar, c2h, dma, log, &|| false, clock)?;
    observe(BatchPhase::Readout(clock.now().saturating_duration_since(started)));
    for step in rest {
        let read =
            if let Step::Read { frames, frame } = step { Some((clock.now(), read_len(*frames, *frame))) } else { None };
        plain_step(step, bar, c2h, dma, log, &|| false, clock)?;
        if let Some((started, bytes)) = read {
            observe(BatchPhase::Read(clock.now().saturating_duration_since(started), bytes));
        }
    }
    Ok(())
}

/// Runs steps that need only the register window and the C2H stream (no AFE step) on any `Bus`: the unit, or a model
/// of it in the tests. The Read step goes into `dma` like `execute_into_until`.
pub fn execute_bus<B: crate::dho::bus::Bus, R: std::io::Read>(
    steps: &[Step],
    bar: &mut B,
    c2h: &mut R,
    dma: &mut crate::dho::bus::DmaBuf,
    mut log: impl FnMut(String),
) -> Result<(), String> {
    execute_bus_until(steps, bar, c2h, dma, &mut log, &|| false)
}

/// `execute_bus` whose polls end early with `Err(NEW_SETTINGS)` when `stop()` says so (checked every 10 ms).
pub fn execute_bus_until<B: crate::dho::bus::Bus, R: std::io::Read>(
    steps: &[Step],
    bar: &mut B,
    c2h: &mut R,
    dma: &mut crate::dho::bus::DmaBuf,
    log: &mut impl FnMut(String),
    stop: &impl Fn() -> bool,
) -> Result<(), String> {
    execute_with_clock(steps, bar, c2h, dma, log, stop, &time::Real)
}

fn execute_with_clock<B: crate::dho::bus::Bus, R: std::io::Read>(
    steps: &[Step],
    bar: &mut B,
    c2h: &mut R,
    dma: &mut crate::dho::bus::DmaBuf,
    log: &mut impl FnMut(String),
    stop: &impl Fn() -> bool,
    clock: &impl Clock,
) -> Result<(), String> {
    let mut steps = steps.iter().peekable();
    while let Some(s) = steps.next() {
        plain_step(s, bar, c2h, dma, log, stop, clock)?;
        if matches!(s, Step::Wr(_)) && !matches!(steps.peek(), Some(Step::Wr(_))) {
            bar.settle();
        }
    }
    Ok(())
}

/// One step of `execute_bus` / `execute_into_until` other than the AFE steps.
fn plain_step<B: crate::dho::bus::Bus, R: std::io::Read>(
    s: &Step,
    bar: &mut B,
    c2h: &mut R,
    dma: &mut crate::dho::bus::DmaBuf,
    log: &mut impl FnMut(String),
    stop: &impl Fn() -> bool,
    clock: &impl Clock,
) -> Result<(), String> {
    match s {
        Step::Wr((off, v)) => bar.wr(*off, *v),
        Step::Afe(_) => return Err("AFE step without AFE bus".into()),
        Step::Poll { .. } => poll(s, bar, log, stop, clock)?,
        Step::Read { frames, frame } => {
            let bytes = read_len(*frames, *frame);
            let buf = dma
                .read(c2h, bytes, |got, k| log(format!("c2h: short read of {} bytes (at {} of {})", k, got, bytes)))
                .map_err(|e| format!("c2h read of {} bytes: {} (0x4034 = {:#x})", bytes, e, bar.rd(0x4034)))?;
            let n = unpad_in(buf, *frame);
            dma.truncate(n);
        }
        Step::Peek(regs) => {
            log(regs.iter().map(|r| format!("{:#06x} = {:#010x}", r, bar.rd(*r))).collect::<Vec<_>>().join(", "))
        }
        Step::Sleep { us } => clock.wait(std::time::Duration::from_micros(*us as u64)),
        Step::Note(n) => log(n.clone()),
    }
    Ok(())
}

/// A second read of frame `index` of the batch just read with FPGA compression (`s.compress` > 1), this time raw: the
/// same record, every sample (fork, docs/fork/compress.md). The ring is played again as for the LA read and the stamp
/// plays of hardware segments; nothing may arm or write the FPGA in between. The frame lands in `dma`
/// (`frame_bytes` of `raw_settings(s)`).
#[allow(clippy::too_many_arguments)] // Shares the acquisition register, DMA and logging ports.
pub fn read_raw_again<B: crate::dho::bus::Bus, R: std::io::Read>(
    st: &mut State,
    s: &Settings,
    p: &hz::Params,
    bar: &mut B,
    c2h: &mut R,
    dma: &mut crate::dho::bus::DmaBuf,
    index: u32,
    log: impl FnMut(String),
) -> Result<(), String> {
    execute_bus(&st.read_steps_at(&raw_settings(s), p, index, 1), bar, c2h, dma, log)
}
