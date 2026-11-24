// SPDX-License-Identifier: GPL-2.0-or-later

use super::super::{
    batch, batch_plan, batch_timing,
    comparator::{is_stall, logs_acquire_error, mask_map, unit_key},
    dma_source::{self, ArmPlan},
    lifecycle,
    window::prearm_keeps_replay,
    Settings,
};
use super::dispatch::{Handoff, Published};
use super::pacing::PacingPolicy;
use super::read::{Capture, ReadIo};
use super::{replay, status, AcqLoop, BatchState, Io, LoopClock, Port, Replay, Request};
use crate::sysconf::Switches;
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
        if std::mem::take(&mut self.replay.paused) {
            lock(&self.out).clear_queue(drawpipe::Lost::Settings);
        }
        let mut b = batch_plan(settings, params, self.switches.board.zone_batch);
        let (deposit, uncapped) = batch::deposit_plan(&b, params, request.identity, request.route);
        let uncapped = request.run.batch_bound(uncapped);
        let grant = batch::grant(
            (deposit, uncapped),
            &self.batch.cap,
            (&self.out, request.identity),
            (&b, params),
            self.switches.source.work_budget(),
        );
        dma_source::limit_frames(
            &mut b,
            grant.records(),
            self.batch.armed.as_ref(),
            (request.pair(), request.route, deposit),
        );
        self.clear_average(io, request.avg_clear);
        let plan = ArmPlan::new(&b, request.identity, request.route, deposit, grant.serial());
        let out = &self.out;
        let obsolete = || plan.obsolete(out, (request.want.as_ref(), request.mode));
        let Some((ahead, tap, timing)) = Self::arm(
            (&mut self.batch, &mut self.replay),
            io,
            out,
            (&b, params),
            (&plan, deposit, grant, uncapped > 0),
            obsolete,
        ) else {
            self.replay.fresh = last_read && self.replay.last_batch.is_some();
            return;
        };
        let capture = Capture { settings: &b, params, request: &request, uncapped };
        self.capture(io, capture, (&plan, deposit), (ahead, tap, timing, last_read));
    }

    pub(super) fn capture(
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
                self.replay.invalidate(out, seq::frames(b, params));
                self.replay.fresh = last_read && self.replay.last_batch.is_some();
                self.batch.failed(&read_io, &capture, out, error);
                return;
            }
        };
        let unit = read_io.state.mask_ctrl().is_some() || read_io.state.zone_ctrl().is_some();
        self.batch.guard.batch(false, unit, unit_key(b, mask_map()), read_io.clock.now());
        let readout =
            self.batch.read(&mut read_io, &capture, &completed, (out, self.switches.board.header_probe), tap.as_ref());
        let record = (completed.count - 1, readout.raw_frame.is_some());
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
                io.disarm(&mut self.batch.armed, &mut self.replay, out);
                self.batch.prearm_timing = None;
            }
        }
    }

    fn clear_average(&mut self, io: &mut Io<impl Port>, clear: u64) {
        if let Some((tracker, seen, logged)) = self.batch.avg.as_mut().filter(|avg| avg.1 != clear) {
            io.disarm(&mut self.batch.armed, &mut self.replay, &self.out);
            let steps = io.state.avg_clear_steps();
            if let Err(e) = io.execute(&steps) {
                status(&self.out, format!("average clear: {}", e));
            }
            tracker.reset();
            (*seen, *logged) = (clear, 0);
        }
    }

    fn arm<P: Port>(
        (batch, replay): (&mut BatchState, &mut Replay),
        io: &mut Io<P>,
        out: &HwShared,
        (b, p): (&Settings, &hz::Params),
        (plan, deposit, grant, has_records): (&ArmPlan<'_>, Option<&Pipe>, drawpipe::admission::Grant, bool),
        obsolete: impl Fn() -> bool,
    ) -> Option<(bool, Reserved, batch_timing::SourceTiming)> {
        let records_can_arm = plan.read_ahead();
        if !has_records
            || (!records_can_arm
                && batch.record_wait.poll(out, deposit, &io.clock, &obsolete, |message| status(out, message)))
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
        let clock = &io.clock;
        let (mut ahead, tap) = plan.prepare(&mut batch.dma, &mut batch.armed, clock, obsolete, |armed| {
            armed.disarm(replay, out, || super::disarm(&mut io.state, &mut io.bar, clock, out));
        })?;
        let prearm = batch.prearm_timing.take().filter(|_| ahead);
        let mut timing = batch_timing::SourceTiming::new(prearm, grant);
        // Force uses a single pulse after arming, except when a hardware average counts triggers.
        let force = std::mem::take(&mut lock(out).force);
        if force && batch.avg.is_none() && (ahead || batch.arm_for_force(io, out, (b, p), &mut timing)) {
            io.bar.apply(&oos_hal::dho::tpu::force(&io.state.ts));
            ahead = true;
        }
        Some((ahead, tap, timing))
    }
}

impl BatchState {
    fn arm_for_force(
        &mut self,
        io: &mut Io<impl Port>,
        out: &HwShared,
        (settings, params): (&Settings, &hz::Params),
        timing: &mut batch_timing::SourceTiming,
    ) -> bool {
        timing
            .arm(&io.clock, || {
                lifecycle::prearm(settings.single, || {
                    let steps = io.state.arm_only_steps(settings, params);
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
            .is_ok()
    }

    pub(super) fn configured(&mut self, mode: Mode, kind: avg::Kind, avg_clear: u64) {
        self.avg = match mode {
            Mode::Average { count } => Some((avg::Tracker::new(kind, count), avg_clear, 0)),
            _ => None,
        };
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
