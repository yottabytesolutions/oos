// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later
//! Generic acquisition and step execution; the clock is supplied at the IO boundary.
use super::*;

/// One batch on the unit: arm `frames(s, p)` frames, wait for the first, then let the ring fill until all frames are
/// recorded or `wait_ms` have passed since the first one (0x4010 reads back the last recorded frame, so the count is
/// `0x4010 - first + 1`), then play and read exactly those frames into `dma`.
/// `armed`: `prearm` already armed this batch with the same `s` and `p`; only the wait is left.
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
    acquire_batch_with_clock(st, s, p, bar, c2h, dma, armed, wait_ms, log, stop, idle, &super::time::Real)
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
    mut log: impl FnMut(String),
    stop: impl Fn() -> bool,
    mut idle: impl FnMut() -> bool,
    clock: &impl super::time::Clock,
) -> Result<u32, String> {
    let nf = frames(s, p);
    let arm = if armed { wait_steps(p) } else { st.arm_steps(s, p) };
    execute_with_clock(&arm, bar, c2h, dma, &mut log, &stop, clock)?;
    let t = clock.now();
    let mut count = bar.rd(0x4010).wrapping_add(1).min(nf);
    while count < nf && clock.now().saturating_duration_since(t).as_millis() < wait_ms as u128 {
        if !idle() {
            clock.wait(std::time::Duration::from_micros(200));
        }
        count = bar.rd(0x4010).wrapping_add(1).clamp(1, nf);
    }
    log(format!(
        "{} of {} frames after {:.1} ms",
        count,
        nf,
        clock.now().saturating_duration_since(t).as_secs_f64() * 1e3
    ));
    execute_with_clock(&st.read_steps(s, p, count), bar, c2h, dma, &mut log, &|| false, clock)?;
    Ok(count)
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
    execute_with_clock(steps, bar, c2h, dma, log, stop, &super::time::Real)
}

fn execute_with_clock<B: crate::dho::bus::Bus, R: std::io::Read>(
    steps: &[Step],
    bar: &mut B,
    c2h: &mut R,
    dma: &mut crate::dho::bus::DmaBuf,
    log: &mut impl FnMut(String),
    stop: &impl Fn() -> bool,
    clock: &impl super::time::Clock,
) -> Result<(), String> {
    for s in steps {
        plain_step(s, bar, c2h, dma, log, stop, clock)?;
    }
    Ok(())
}

/// One step of `execute_bus` / `execute_into_until` other than the AFE steps.
pub(super) fn plain_step<B: crate::dho::bus::Bus, R: std::io::Read>(
    s: &Step,
    bar: &mut B,
    c2h: &mut R,
    dma: &mut crate::dho::bus::DmaBuf,
    log: &mut impl FnMut(String),
    stop: &impl Fn() -> bool,
    clock: &impl super::time::Clock,
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

fn poll<B: crate::dho::bus::Bus>(
    step: &Step,
    bar: &mut B,
    log: &mut impl FnMut(String),
    stop: &impl Fn() -> bool,
    clock: &impl super::time::Clock,
) -> Result<(), String> {
    let Step::Poll { reg, bit, ms, what } = step else { return Ok(()) };
    let t = clock.now();
    let mut last = bar.rd(*reg);
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
