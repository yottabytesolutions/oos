// SPDX-License-Identifier: GPL-2.0-or-later

use super::{execute_bus_with_clock, Clock, State};
use crate::dho::{
    hz,
    seq::{raw_settings, Settings},
};

/// The register, stream, DMA and clock ports used to replay a raw frame.
pub struct RawReadIo<'a, B, R, C> {
    bar: &'a mut B,
    stream: &'a mut R,
    dma: &'a mut crate::dho::bus::DmaBuf,
    clock: &'a C,
}

impl<B, R, C> std::fmt::Debug for RawReadIo<'_, B, R, C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RawReadIo").field("dma_len", &self.dma.data().len()).finish_non_exhaustive()
    }
}

impl<'a, B, R, C> RawReadIo<'a, B, R, C> {
    pub fn new(bar: &'a mut B, stream: &'a mut R, dma: &'a mut crate::dho::bus::DmaBuf, clock: &'a C) -> Self {
        Self { bar, stream, dma, clock }
    }
}

/// Replay frame `index` of the current batch without FPGA compression, using the supplied clock.
/// Nothing may arm or write the FPGA between the original read and replay. The frame lands in the supplied DMA
/// buffer (`frame_bytes` of `raw_settings(s)`).
pub fn read_raw_again_with_clock(
    st: &mut State,
    s: &Settings,
    p: &hz::Params,
    io: RawReadIo<'_, impl crate::dho::bus::Bus, impl std::io::Read, impl Clock>,
    index: u32,
    mut log: impl FnMut(String),
) -> Result<(), String> {
    let steps = st.reread_steps(&raw_settings(s), p, index).map_err(|error| error.to_string())?;
    execute_bus_with_clock(&steps, io.bar, io.stream, io.dma, &mut log, &|| false, io.clock)
}

/// Reads LA words using the supplied clock, restoring the analog stream after a failed read.
pub fn acquire_la_with_clock(
    st: &mut State,
    s: &Settings,
    p: &hz::Params,
    bar: &mut impl crate::dho::bus::Bus,
    c2h: &mut impl std::io::Read,
    mut log: impl FnMut(String),
    clock: &impl Clock,
) -> Result<Vec<u16>, String> {
    use crate::dho::la;
    let (n, _) = la::window(&la::timing(p, s.mask));
    let mut dma = crate::dho::bus::DmaBuf::new();
    match execute_bus_with_clock(&st.la_read_steps(s, p), bar, c2h, &mut dma, &mut log, &|| false, clock) {
        Ok(()) => Ok(la::words(dma.data(), n as usize)),
        Err(e) => {
            // The read failed before the analog stream was restored.
            bar.apply(&la::restore(&mut st.sh));
            Err(e)
        }
    }
}
