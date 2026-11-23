// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! Execution on the unit, including the analog front end and LA.

use super::{execute_bus, plain_step, time, Settings, State, Step};
use crate::dho::bus::{Bus, DmaBuf};
use crate::dho::{afe, hz, la};

/// Arms the next batch right after a read (and the LA read of the same record), so the FPGA records while the host
/// decodes: the same writes as the arm `acquire_batch` would issue next, earlier. Read it with `acquire_batch(..,
/// armed = true, ..)` and the same `s` and `p`; anything else that writes the FPGA first needs `disarm`.
pub fn prearm(
    st: &mut State,
    s: &Settings,
    p: &hz::Params,
    bar: &mut impl Bus,
    c2h: &mut std::fs::File,
    log: impl FnMut(String),
) -> Result<(), String> {
    execute_bus(&st.arm_only_steps(s, p), bar, c2h, &mut DmaBuf::new(), log)
}

/// Stops an acquisition armed by `prearm` that will not be read (`State::disarm_steps`).
pub fn disarm(
    st: &mut State,
    bar: &mut crate::dho::bus::Bar1,
    c2h: &mut std::fs::File,
    log: impl FnMut(String),
) -> Result<(), String> {
    execute(&st.disarm_steps(), bar, None, c2h, log).map(|_| ())
}

/// The LA read of the acquisition just read out (`la_read_steps`) on the unit: LA words, bit n = Dn, word `start` of
/// `la::window` first.
pub fn acquire_la(
    st: &mut State,
    s: &Settings,
    p: &hz::Params,
    bar: &mut impl Bus,
    c2h: &mut std::fs::File,
    log: impl FnMut(String),
) -> Result<Vec<u16>, String> {
    let (n, _) = la::window(&la::timing(p, s.mask));
    let mut dma = DmaBuf::new();
    match execute_bus(&st.la_read_steps(s, p), bar, c2h, &mut dma, log) {
        Ok(()) => Ok(la::words(dma.data(), n as usize)),
        Err(e) => {
            // the read failed before `la::restore` ran: without it the stream stays on the LA and the analog is gone
            bar.apply(&la::restore(&mut st.sh));
            Err(e)
        }
    }
}

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
    for s in steps {
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
            _ => plain_step(s, bar, c2h, dma, &mut log, &stop, &time::Real)?,
        }
    }
    Ok(())
}
