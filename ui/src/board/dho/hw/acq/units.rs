// SPDX-License-Identifier: GPL-2.0-or-later

use super::super::{
    dma_source::ArmedBatch,
    segmented::SegRun,
    trim::{self, TrimPending},
    units::{dvm_reading, dvm_step, DvmKey, DvmStep},
};
use super::{Io, Port, Request};
use crate::{
    board::{HwShared, Slow},
    perfguard::deep::acq_lock as lock,
};
use oos_hal::dho::{
    bus::Bus,
    dvm,
    seq::{time::Clock, Settings},
};
use std::time::{Duration, Instant};

#[derive(Default)]
pub(super) struct Units {
    trim_done: [bool; 11],
    trim_tries: [u8; 11],
    trim_pending: Option<TrimPending>,
    dvm_run: Option<(DvmKey, Instant)>,
    dvm_n: u64,
    configs: u64,
    slots_done: Option<u64>,
}

impl Units {
    pub(super) fn reset_trim(&mut self) {
        (self.trim_done, self.trim_tries) = ([false; 11], [0; 11]);
    }

    pub(super) fn configured(&mut self, mode: usize, mask: u32, tight: bool, now: Instant) {
        self.configs += 1;
        self.trim_pending = (tight && !self.trim_done[mode] && self.trim_tries[mode] < trim::TRIM_TRIES)
            .then_some(TrimPending { mode, mask, since: now });
    }

    pub(super) fn poll<P: Port>(
        &mut self,
        io: &mut Io<P>,
        (armed, segments): (&mut Option<ArmedBatch>, Option<&SegRun>),
        out: &HwShared,
        (settings, mode): (&Settings, Slow),
        (request, generator_on): (&Request, bool),
    ) {
        if segments.is_some_and(SegRun::recording) {
            return;
        }
        self.dvm(io, armed, out, settings, request.dvm);
        if mode != Slow::Off {
            return;
        }
        if let Some(pending) = self.trim_pending.filter(|t| !request.run.hold && t.due(io.clock.now())) {
            self.trim_pending = None;
            if !generator_on && pending.mode == io.state.mode.min(10) && io.cal_adc.is_some() {
                io.disarm(armed);
                self.trim_tries[pending.mode] += 1;
                self.trim_done[pending.mode] = P::trim(io, pending.mask);
            }
        }
        if let Some(id) = request.slots.filter(|&id| self.slots_done != Some(id)) {
            io.disarm(armed);
            let result =
                oos_hal::dho::levels::run(&mut io.bar, &mut |us| io.clock.wait(Duration::from_micros(us.into())));
            lock(out).slots =
                Some((id, result.map(|slots| slots.map(|s| [s.min as f32, s.max as f32, s.mean as f32]))));
            self.slots_done = Some(id);
        }
    }

    fn dvm(
        &mut self,
        io: &mut Io<impl Port>,
        armed: &mut Option<ArmedBatch>,
        out: &HwShared,
        settings: &Settings,
        want: Option<crate::board::DvmWant>,
    ) {
        let elapsed = self.dvm_run.map_or(0.0, |run| io.clock.now().duration_since(run.1).as_secs_f64());
        let (step, key) = dvm_step(want, settings.mask, self.configs, self.dvm_run.map(|run| run.0), elapsed);
        let reading = match step {
            DvmStep::Idle => return,
            DvmStep::Start(lane) => {
                io.disarm(armed);
                io.bar.apply(&dvm::start(lane, dvm::EXP));
                None
            }
            DvmStep::Stop => {
                io.disarm(armed);
                io.bar.apply(&dvm::stop());
                None
            }
            DvmStep::Read => {
                let raw = dvm::read(&mut io.bar);
                self.dvm_n += 1;
                key.map(|key| dvm_reading(&raw, key, settings, &io.state.gain, self.dvm_n))
            }
        };
        lock(out).dvm = reading;
        self.dvm_run = key.map(|key| (key, io.clock.now()));
    }
}
