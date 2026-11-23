// SPDX-License-Identifier: GPL-2.0-or-later

use super::super::{
    batch, batch_plan, batch_timing,
    comparator::{is_stall, logs_acquire_error, mask_map, unit_key},
    dma_source::{self, ArmPlan},
    lifecycle, single, stop,
    window::prearm_keeps_replay,
    Settings,
};
use super::dispatch::{Handoff, Published};
use super::pacing::PacingPolicy;
use super::read::{Capture, ReadIo};
use super::{replay, status, AcqLoop, BatchState, Io, LoopClock, Port, Request};
use crate::sysconf::{switches::StopSlot, Switches};
use crate::{
    board::HwShared,
    drawpipe::{self, Pipe, TapReservation},
    perfguard::deep::acq_lock as lock,
};
use oos_hal::dho::{
    acq::Mode,
    avg,
    bus::{Bus, DmaBuf},
    hz,
    seq::{self, time::Clock},
};
use std::{io::Read, sync::Arc, time::Duration};

pub(super) type Reserved = Option<Arc<TapReservation>>;

impl<T: PacingPolicy> AcqLoop<T> {
    pub(super) fn batch(
        &mut self,
        io: &mut Io<impl Port>,
        (settings, params): (&Settings, &hz::Params),
        request: Request,
        last_read: bool,
    ) {
        let out = &self.out;
        if !self.batch.ready(out, settings, &request, &mut self.replay.paused, &io.clock) {
            return;
        }
        let mut b = batch_plan(settings, params, self.switches.board.zone_batch);
        let (deposit, uncapped) = batch::deposit_plan(&b, params, request.identity, request.route);
        let uncapped = request.run.batch_bound(uncapped);
        let grant = batch::grant(
            (deposit, uncapped),
            &self.batch.cap,
            (out, request.identity),
            (&b, params),
            self.switches.source.work_budget(),
        );
        dma_source::limit_frames(
            &mut b,
            grant.records(),
            self.batch.armed.as_ref(),
            (request.pair(), request.route, deposit),
        );
        if !self.batch.single_ready(io, settings, &request) {
            return;
        }
        self.batch.clear_average(io, out, request.avg_clear);
        let plan = ArmPlan::new(&b, request.identity, request.route, deposit, grant.serial());
        let obsolete = || plan.obsolete(out, (request.want.as_ref(), request.mode));
        let previous = self.replay.last_batch.as_ref().map(|batch| batch.index);
        let Some((ahead, first, tap, timing)) = self.batch.arm(
            io,
            out,
            (&b, params),
            (&plan, deposit, grant, uncapped > 0),
            (self.switches.source.stop_slot(), previous),
            obsolete,
        ) else {
            self.replay.fresh = last_read;
            return;
        };
        let capture = Capture { settings: &b, params, request: &request, uncapped, first };
        self.capture(io, capture, (&plan, deposit), (ahead, tap, timing, last_read));
    }

    fn capture(
        &mut self,
        io: &mut Io<impl Port>,
        capture: Capture<'_>,
        (plan, deposit): (&ArmPlan<'_>, Option<&'static Pipe>),
        (ahead, tap, timing, last_read): (bool, Reserved, batch_timing::SourceTiming, bool),
    ) {
        let out = &self.out;
        let Capture { settings: b, params, request, .. } = capture;
        let obsolete = || plan.handoff_obsolete(&lock(out), (request.want.as_ref(), request.mode));
        let mut read_io = io.batch_io();
        let completed = match self.batch.fill(
            &mut read_io,
            &capture,
            (ahead, timing, &self.pacing),
            (out, tap.is_some(), self.switches.board.prearm && !plan.serial()),
            plan,
        ) {
            Ok(completed) => completed,
            Err(error) => {
                let frames = if !plan.serial() && (error == seq::NEW_SETTINGS || is_stall(&error)) {
                    1
                } else {
                    seq::frames(b, params)
                };
                self.replay.fresh = stop::retain_replay(&mut self.replay.last_batch, capture.first, frames, last_read);
                self.batch.failed(&read_io, &capture, out, error);
                return;
            }
        };
        let unit = read_io.state.mask_ctrl().is_some() || read_io.state.zone_ctrl().is_some();
        self.batch.guard.batch(false, unit, unit_key(b, mask_map()), read_io.clock.now());
        let readout =
            self.batch.read(&mut read_io, &capture, &completed, (out, self.switches.board.header_probe), tap.as_ref());
        let record = (capture.first + completed.count - 1, readout.raw_frame.is_some());
        let batch = replay::Batch::new(b.clone(), *params, record, request.identity, readout.la.clone());
        (self.replay.last_batch, self.replay.fresh) = (Some(batch), true);
        let handoff = Handoff { capture, completed, plan, deposit, tap, readout };
        let published = self.batch.publish(&mut read_io, handoff, (out, &self.switches), &mut self.seq, obsolete);
        drop(read_io);
        match published {
            Published::Running => {}
            Published::Held => {
                self.replay.paused = true;
                self.replay.fresh = false;
            }
            Published::Rejected => {
                if self.batch.armed.is_some() {
                    (self.replay.last_batch, self.replay.fresh) = (None, false);
                }
                io.disarm(&mut self.batch.armed);
                self.batch.prearm_timing = None;
            }
        }
    }
}

impl BatchState {
    pub(super) fn configured(&mut self, mode: Mode, kind: avg::Kind, avg_clear: u64) {
        self.avg = match mode {
            Mode::Average { count } => Some((avg::Tracker::new(kind, count), avg_clear, 0)),
            _ => None,
        };
    }

    fn ready(
        &mut self,
        out: &HwShared,
        settings: &Settings,
        request: &Request,
        paused: &mut bool,
        clock: &impl Clock,
    ) -> bool {
        if single::progress(settings, self.single_done, request.pair(), request.run.hold, 0).1 == single::Poll::Hold {
            lock(out).final_ack = request.run.final_req;
            clock.wait(Duration::from_millis(5));
            return false;
        }
        if std::mem::take(paused) {
            lock(out).clear_queue(drawpipe::Lost::Settings);
        }
        true
    }

    fn single_ready(&mut self, io: &mut Io<impl Port>, settings: &Settings, request: &Request) -> bool {
        if single::progress(settings, self.single_done, request.pair(), false, 0).1 == single::Poll::Wait {
            io.disarm(&mut self.armed);
            io.clock.wait(Duration::from_millis(2));
            return false;
        }
        true
    }

    fn clear_average(&mut self, io: &mut Io<impl Port>, out: &HwShared, clear: u64) {
        if let Some((tracker, seen, logged)) = self.avg.as_mut().filter(|avg| avg.1 != clear) {
            io.disarm(&mut self.armed);
            let steps = io.state.avg_clear_steps();
            if let Err(e) = io.execute(&steps) {
                status(out, format!("average clear: {}", e));
            }
            tracker.reset();
            (*seen, *logged) = (clear, 0);
        }
    }

    fn arm<P: Port>(
        &mut self,
        io: &mut Io<P>,
        out: &HwShared,
        (b, p): (&Settings, &hz::Params),
        (plan, deposit, grant, has_records): (&ArmPlan<'_>, Option<&Pipe>, drawpipe::admission::Grant, bool),
        (slot, previous): (StopSlot, Option<u32>),
        obsolete: impl Fn() -> bool,
    ) -> Option<(bool, u32, Reserved, batch_timing::SourceTiming)> {
        let records_can_arm = plan.read_ahead();
        if !has_records
            || (!records_can_arm
                && self.record_wait.poll(out, deposit, &io.clock, &obsolete, |message| status(out, message)))
        {
            return None;
        }
        {
            let mut g = lock(out);
            if g.hold {
                return None;
            }
            g.record_flow.serial = plan.serial();
        }
        let (mut ahead, tap) = plan.prepare(&mut self.dma, &mut self.armed, &io.clock, obsolete, |armed| {
            super::disarm(armed, &mut io.state, &mut io.bar, &mut io.c2h, &io.clock);
        })?;
        let first = stop::first_frame(slot, b, p, previous, plan.serial() && !ahead);
        let prearm = self.prearm_timing.take().filter(|_| ahead);
        let mut timing = batch_timing::SourceTiming::new(prearm, grant);
        // Force uses a single pulse after arming, except when a hardware average counts triggers.
        if std::mem::take(&mut lock(out).force) && self.avg.is_none() {
            let armed = ahead
                || timing
                    .arm(&io.clock, || {
                        lifecycle::prearm(b.single, || {
                            let steps = if first == 0 {
                                io.state.arm_only_steps(b, p)
                            } else {
                                io.state.arm_only_steps_at(b, p, first)?
                            };
                            seq::time::execute_bus_with_clock(
                                &steps,
                                &mut io.port.batch_bus(&mut io.bar),
                                &mut io.c2h,
                                self.dma.destination(),
                                &mut |_| {},
                                &|| false,
                                &io.clock,
                            )
                        })
                    })
                    .map_err(|e| status(out, format!("force: arm: {}", e)))
                    .is_ok();
            if armed {
                io.bar.apply(&oos_hal::dho::tpu::force(&io.state.ts));
                ahead = true;
            }
        }
        Some((ahead, first, tap, timing))
    }

    fn failed(
        &mut self,
        io: &ReadIo<'_, impl Bus, impl Read, impl LoopClock>,
        capture: &Capture<'_>,
        out: &HwShared,
        error: String,
    ) {
        if error == seq::NEW_SETTINGS {
            return;
        }
        let unit = io.state.mask_ctrl().is_some() || io.state.zone_ctrl().is_some();
        let stalled = is_stall(&error);
        if logs_acquire_error(stalled, unit) {
            status(out, format!("acquire: {}", error));
        }
        if self.guard.batch(stalled, unit, unit_key(capture.settings, mask_map()), io.clock.now()) {
            status(out, "mask/zone unit: no acquisition twice, software fallback for these settings".into());
        }
        if !lock(out).hold {
            io.clock.wait(Duration::from_millis(200));
        }
    }

    pub(super) fn successor(
        &mut self,
        io: &mut ReadIo<'_, impl Bus, impl Read, impl LoopClock>,
        handoff: &Handoff<'_>,
        pipe: Option<&Pipe>,
        (out, switches): (&HwShared, &Switches),
        obsolete: impl Fn() -> bool,
    ) -> Result<Option<DmaBuf>, ()> {
        let Capture { settings: b, params: p, request, uncapped, .. } = handoff.capture;
        let next = (switches.board.prearm
            && !handoff.plan.serial()
            && self.avg.is_none()
            && !b.single
            && request.run.segment_bound.is_none()
            && prearm_keeps_replay(b, p, request.run.deep))
        .then(|| {
            batch::grant(
                (handoff.deposit, uncapped),
                &self.cap,
                (out, request.identity),
                (b, p),
                switches.source.work_budget(),
            )
        });
        let clock = io.clock;
        handoff.plan.prearm_and_wait(
            pipe,
            clock,
            || {
                handoff.plan.prearm(&mut self.dma, &mut self.armed, pipe, next, &obsolete, |successor, _| {
                    let interrupted = {
                        let g = lock(out);
                        g.force || g.cal_req.is_some()
                    };
                    if interrupted {
                        return false;
                    }
                    batch_timing::timed_arm(&mut self.prearm_timing, clock, || {
                        lifecycle::prearm(successor.single, || {
                            let steps = io.state.arm_only_steps(successor, p);
                            io.execute(&steps, &mut DmaBuf::new())
                        })
                    })
                    .map_err(|e| status(out, format!("prearm: {}", e)))
                    .is_ok()
                })
            },
            &obsolete,
        )
    }
}
