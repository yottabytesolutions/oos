// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! Execution on the unit, including the analog front end and LA.

use super::{plain_step, time, Step};
use crate::dho::{afe, bus::Bus};

/// Runs steps on the unit; returns the bytes of the Read step. `log` gets notes and poll results.
pub fn execute(
    steps: &[Step],
    bar: &mut crate::dho::bus::Bar1,
    afe_bus: Option<&mut afe::GpioAfe>,
    c2h: &mut std::fs::File,
    log: impl FnMut(String),
) -> Result<Vec<u8>, String> {
    let mut dma = crate::dho::bus::DmaBuf::new();
    execute_into_until(steps, bar, afe_bus, c2h, &mut dma, log, || false)?;
    Ok(dma.into_vec())
}

/// `execute` whose polls end early with `Err(NEW_SETTINGS)` when `stop()` says so (checked every 10 ms).
pub fn execute_until(
    steps: &[Step],
    bar: &mut crate::dho::bus::Bar1,
    afe_bus: Option<&mut afe::GpioAfe>,
    c2h: &mut std::fs::File,
    log: impl FnMut(String),
    stop: impl Fn() -> bool,
) -> Result<Vec<u8>, String> {
    let mut dma = crate::dho::bus::DmaBuf::new();
    execute_into_until(steps, bar, afe_bus, c2h, &mut dma, log, stop)?;
    Ok(dma.into_vec())
}

/// `execute` with the Read step into `dma` (kept across batches): the bytes are `dma.data()`, without padding. Polls
/// end early like `execute_until`.
pub fn execute_into_until(
    steps: &[Step],
    bar: &mut crate::dho::bus::Bar1,
    afe_bus: Option<&mut afe::GpioAfe>,
    c2h: &mut std::fs::File,
    dma: &mut crate::dho::bus::DmaBuf,
    mut log: impl FnMut(String),
    stop: impl Fn() -> bool,
) -> Result<(), String> {
    let mut afe_bus = afe_bus;
    for (index, s) in steps.iter().enumerate() {
        match s {
            Step::Afe(afe::Op::Afe { ch, reg, val }) => {
                use afe::AfeBus;
                afe_bus.as_deref_mut().ok_or("AFE step without AFE bus")?.write(*ch, *reg, *val)
            }
            Step::Afe(afe::Op::Gear { ch, gear }) => afe::set_gear(*ch, *gear).map_err(|e| format!("gear: {}", e))?,
            Step::Afe(afe::Op::Adc(w)) => {
                std::thread::sleep(std::time::Duration::from_micros(100));
                bar.wr(0x1034, *w);
                std::thread::sleep(std::time::Duration::from_micros(100));
            }
            _ => plain_step((s, &steps[..index]), bar, c2h, dma, &mut log, &stop, &time::Real)?,
        }
    }
    Ok(())
}
