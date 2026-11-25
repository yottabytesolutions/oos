// SPDX-License-Identifier: GPL-2.0-or-later

//! Full-bandwidth readers and draw-only peak batches share one acquisition identity.

use super::super::{
    batch, batch_timing::SourceTiming, dma_source::ArmPlan, header::dump, lifecycle, plan::batch_plan, raw, Settings,
};
use super::{
    pacing::PacingPolicy,
    publication,
    read::{Capture, Completed, ReadIo},
    status,
};
use super::{AcqLoop, BatchState, Io, LoopClock, Port, Request};
use crate::board::dho::clock::Wake;
use crate::drawpipe::{self, admission::Grant, Lost, Pipe};
use crate::perfguard::deep::acq_lock as lock;
use oos_hal::dho::{
    bus::Bus,
    hz,
    seq::{self, time::BatchPhase, time::Clock, Step},
};
use std::{io::Read, time::Duration};

mod frames;
mod offsets;

#[cfg(test)]
pub(in crate::board::dho::hw) use frames::fixture_profile;

pub(super) use frames::Decoders;
use frames::{decode_reference, submit_profile};

pub(super) fn plan_capture(cycle: &mut raw::ReferenceCycle, request: &Request, want: &mut Settings, auto: bool) {
    if !auto && !raw::is_reduced(want) {
        return;
    }
    let pipe = pipe(request).filter(|_| {
        !request.needs_raw && want.hwmask.is_none() && want.hwzone.is_none() && request.run.segment_bound.is_none()
    });
    if let Some(pipe) = pipe.filter(|_| auto) {
        drawpipe::with_plot(pipe, request.identity, |model, columns| {
            super::super::read_plan::auto_capture(model, request.mode, want, columns);
        });
    }
    if raw::is_reduced(want) {
        let pair = request.pair();
        cycle.track(pair);
        if pipe.is_none() || !cycle.can_reduce(pair) {
            want.mode = oos_hal::dho::acq::Mode::Normal;
        }
    }
}

pub(super) fn waiting_for_picture(clock: &impl LoopClock) -> bool {
    let Some(pipe) = drawpipe::pipe() else { return false };
    let generation = pipe.generation();
    if !pipe.in_flight() {
        return false;
    }
    // The UI keeps the view active until STOP's acknowledgement. Let the final funded tap reach it first.
    clock.park(pipe, generation, Duration::from_millis(100));
    true
}

fn pipe(request: &Request) -> Option<&'static Pipe> {
    drawpipe::pipe().filter(|pipe| request.route && pipe.accepting())
}

pub(super) fn reduced_batch<T: PacingPolicy>(
    source: &mut AcqLoop<T>,
    io: &mut Io<impl Port>,
    (reduced, reduced_params): (&Settings, &hz::Params),
    request: &Request,
) {
    let Some(pipe) = pipe(request) else { return };
    let asked = (request.want.as_ref(), request.mode);
    let gate = ArmPlan::new(reduced, request.identity, request.route, Some(pipe), true);
    let obsolete = || gate.obsolete(&source.out, asked);
    if !gate.wait_to_submit(pipe, &io.clock, obsolete) || !source.batch.dma.ready(request.identity, &io.clock, obsolete)
    {
        return;
    }
    let reference = source.batch.raw.reference.remaining(request.pair(), io.clock.now()).is_zero();
    if reference {
        let settings = raw::reference_settings(reduced);
        let params = hz::prep(settings.mask, seq::tdiv_fs(settings.tdiv), settings.depth, 0);
        capture_profile(source, io, &settings, &params, (request, pipe));
    } else {
        capture_profile(source, io, reduced, reduced_params, (request, pipe));
    }
}

fn switch_capture_profile<P: Port>(
    source: &mut AcqLoop<impl PacingPolicy>,
    io: &mut Io<P>,
    settings: &Settings,
    reference: bool,
) -> bool {
    io.disarm(&mut source.batch, &mut source.replay, &source.out);
    let configured = lifecycle::configure(&mut io.state, settings, io.cal.as_ref(), io.cal_adc.as_ref())
        .and_then(|(steps, _)| P::apply_settings(io, &steps));
    match configured {
        Ok(()) => {
            source.batch.raw.reference.applied(reference);
            true
        }
        Err(error) => {
            source.configured.current = None;
            status(&source.out, format!("capture profile: {error}"));
            io.clock.wait(Duration::from_millis(200));
            false
        }
    }
}

fn capture_profile(
    source: &mut AcqLoop<impl PacingPolicy>,
    io: &mut Io<impl Port>,
    settings: &Settings,
    params: &hz::Params,
    (request, pipe): (&Request, &'static Pipe),
) {
    let reference = !raw::is_reduced(settings);
    let mut b = batch_plan(settings, params, source.switches.board.zone_batch);
    let limit = if reference { Some(1) } else { pipe.read_limit(request.identity, Some((params.rate, 2))) };
    let Some(limit) = limit else {
        source.batch.raw.reference.reject(request.pair());
        return;
    };
    if limit == 0 {
        return;
    }
    let uncapped = b.frames.min(limit);
    let grant = batch::grant(
        (Some(pipe), uncapped),
        &source.batch.cap,
        (&source.out, request.identity),
        (&b, params),
        (source.switches.source.work_budget(), || source.pacing.picture_period()),
    );
    b.frames = grant.records();
    let plan = ArmPlan::new(&b, request.identity, request.route, Some(pipe), true);
    let asked = (request.want.as_ref(), request.mode);
    let obsolete = || plan.obsolete(&source.out, asked);
    let tap = if reference {
        if batch::deposit_plan(&b, params, request.identity, request.route).0.is_none() {
            if !obsolete() {
                source.batch.raw.reference.reject(request.pair());
            }
            return;
        }
        let Ok(tap) = batch::source_reservation(Some(pipe), request.identity, obsolete) else { return };
        tap
    } else {
        None
    };
    if source.batch.raw.reference.needs_switch(reference) && !switch_capture_profile(source, io, settings, reference) {
        source.batch.raw.reference.reject(request.pair());
        return;
    }
    let force = {
        let mut g = lock(&source.out);
        if plan.handoff_obsolete(&g, asked) || g.cal_req.is_some() {
            return;
        }
        g.record_flow.serial = true;
        std::mem::take(&mut g.force)
    };
    let capture = Capture { settings: &b, params, request, uncapped };
    read_capture_profile(source, io, capture, (&plan, pipe), (reference, force, tap, grant));
}

fn read_capture_profile(
    source: &mut AcqLoop<impl PacingPolicy>,
    io: &mut Io<impl Port>,
    capture: Capture<'_>,
    (plan, pipe): (&ArmPlan<'_>, &Pipe),
    (reference, force, tap, grant): (bool, bool, super::batch::Reserved, Grant),
) {
    let pair = capture.request.pair();
    let asked = (capture.request.want.as_ref(), capture.request.mode);
    let obsolete = || plan.obsolete(&source.out, asked);
    let mut timing = SourceTiming::new(None, grant);
    let transition = reference || source.batch.first.proven != Some(pair);
    let ahead = !transition
        && force
        && source.batch.arm_for_force(io, &source.out, (capture.settings, capture.params), &mut timing);
    if ahead {
        io.bar.apply(&oos_hal::dho::tpu::force(&io.state.ts));
    }
    let mut io = io.batch_io();
    let completed = if transition {
        read_capture_transition(&mut source.batch, &mut io, &capture, (reference, force), timing, obsolete)
    } else {
        source
            .batch
            .fill(&mut io, &capture, (ahead, timing, &source.pacing), (&source.out, false, false), plan)
            .map(Some)
    };
    let completed = match completed {
        Ok(Some(completed)) => completed,
        Ok(None) => {
            super::disarm(io.state, &mut io.bar, io.clock, &source.out);
            source.batch.first.proven = None;
            source.batch.raw.reference.reject(pair);
            drawpipe::counters().acquisition.add(crate::board::counters::Event::CaptureTransitionTimeout);
            if force {
                let mut g = lock(&source.out);
                if !plan.handoff_obsolete(&g, asked) {
                    g.force = true;
                }
            }
            return;
        }
        Err(error) => {
            source.batch.first.proven = None;
            if transition {
                if !matches!(&error, seq::time::ExecuteError::DmaTimeout(_)) {
                    super::disarm(io.state, &mut io.bar, io.clock, &source.out);
                }
                if error.message() != seq::NEW_SETTINGS {
                    source.batch.raw.reference.reject(pair);
                }
            }
            source.batch.failed(&mut io, &capture, &source.out, error);
            return;
        }
    };
    finish_capture_profile(source, &mut io, capture, completed, (pipe, tap));
}

fn finish_capture_profile(
    source: &mut AcqLoop<impl PacingPolicy>,
    io: &mut ReadIo<'_, impl Bus, impl Read, impl LoopClock>,
    capture: Capture<'_>,
    completed: Completed,
    (pipe, tap): (&Pipe, super::batch::Reserved),
) {
    let pair = capture.request.pair();
    if completed.reference_at.is_some() {
        source.batch.dma.exchange(&mut source.batch.raw.dma);
    }
    let probe = source.switches.board.header_probe;
    let readout = source.batch.read(io, &capture, &completed, (&source.out, probe), tap.as_ref());
    let reference_at = completed.reference_at;
    let id = capture.request.identity;
    readout.readings.publish(&mut lock(&source.out), id, None, Some((completed.count, 0)));
    let submitted = submit_capture_profile(
        &mut source.batch,
        capture,
        completed,
        (pipe, probe, tap),
        (&source.out, &mut source.seq),
    );
    source.batch.first.proven = (submitted && !readout.failed).then_some(pair);
    if let Some(at) = reference_at {
        if submitted {
            source.batch.raw.reference.completed(pair, at);
        } else {
            source.batch.raw.reference.reject(pair);
        }
    }
}

fn read_capture_transition(
    batch: &mut BatchState,
    io: &mut ReadIo<'_, impl Bus, impl Read, impl LoopClock>,
    capture: &Capture<'_>,
    (reference, force): (bool, bool),
    mut timing: SourceTiming,
    obsolete: impl Fn() -> bool,
) -> Result<Option<Completed>, seq::time::ExecuteError> {
    let (s, p) = (capture.settings, capture.params);
    if p.max_frames < 2 {
        return Err("capture transition needs two ring slots".into());
    }
    let clock = io.clock;
    let steps = seq::CaptureReduction::arm_transition(io.state, s, p);
    let started = clock.now();
    let destination = if reference { &mut batch.raw.dma } else { batch.dma.destination() };
    let mut execute = |bar: &mut _, steps: &[Step], cancel: &dyn Fn() -> bool| {
        seq::time::execute_bus_with_clock(steps, bar, io.c2h, destination, &mut |_| {}, &cancel, clock)
    };
    timing.arm(clock, || execute(&mut io.bar, &steps, &obsolete))?;
    if force {
        let force: Vec<_> = oos_hal::dho::tpu::force(&io.state.ts).into_iter().map(Step::Wr).collect();
        execute(&mut io.bar, &force, &obsolete)?;
    }
    let Some(play) = seq::CaptureReduction::read_transition(io.state, s, p, &mut io.bar, clock, obsolete)? else {
        return Ok(None);
    };
    let triggered = clock.now();
    timing.observe(BatchPhase::FirstTrigger(triggered - started));
    timing.observe(BatchPhase::FillStarted(triggered));
    let (transfer, writes) = play.split_last().ok_or("capture transition produced no read steps")?;
    execute(&mut io.bar, writes, &|| false)?;
    let read_at = clock.now();
    execute(&mut io.bar, std::slice::from_ref(transfer), &|| false)?;
    let end = clock.now();
    timing.observe(BatchPhase::Readout(read_at - triggered));
    timing.observe(BatchPhase::Read(end - read_at, destination.data().len()));
    let batch_ms = (end - started).as_secs_f32() * 1e3;
    crate::pacing::batch(|| end, batch_ms, 1, 1, 0, timing.read_ms());
    Ok(Some(Completed {
        count: 1,
        clock: timing.complete(capture.request.identity, end, 1),
        batch_ms,
        reference_at: reference.then_some(triggered),
    }))
}

fn submit_capture_profile(
    batch: &mut BatchState,
    capture: Capture<'_>,
    completed: Completed,
    (pipe, probe, tap): (&Pipe, crate::board::dho::switches::HeaderProbe, super::batch::Reserved),
    (out, sequence): (&crate::board::HwShared, &mut u64),
) -> bool {
    let (s, p, request) = (capture.settings, capture.params, capture.request);
    let id = request.identity;
    let fb = seq::frame_bytes(s, p);
    let count = completed.count as usize;
    let bytes = batch.dma.destination().data();
    let avail = (bytes.len() / fb).min(count);
    let counters = drawpipe::counters();
    publication::read(counters, completed.count, completed.count, avail);
    if avail == 0 {
        return false;
    }
    *sequence += avail as u64;
    if publication::boundary(Some(request.pair()), lock(out).acquisition.identity()) {
        counters.discard(Lost::Settings, avail as u64);
        return false;
    }
    let newest = if completed.reference_at.is_some() {
        let cores = request.run.cores.then_some(&mut batch.raw.cores);
        let Some((frame, uncorrected)) = decode_reference(bytes, (s, p, probe), cores, (id, *sequence, tap.as_ref()))
        else {
            counters.discard(Lost::Error, avail as u64);
            return false;
        };
        counters.metrics.source_uncorrected(id, usize::from(uncorrected));
        Some(frame)
    } else {
        None
    };
    let pending = batch.raw.dump.as_mut().and_then(|dump| {
        let input = dump::BatchInput::new(
            (s, p),
            (count, avail, crate::board::dho::switches::FrameZero::Keep),
            (&batch.raw.cores, request.run.cores),
        );
        dump.capture_live(request.pair(), input, bytes, (None, None))
    });
    let mut dma = batch.dma.completed().take().expect("the completed capture owns its DMA buffer");
    if newest.is_none() {
        let uncorrected = !request.run.cores || offsets::correct(dma.data_mut(), fb, s, p, &batch.raw.cores);
        if uncorrected {
            counters.metrics.source_uncorrected(id, avail);
        }
    }
    let callbacks = batch.raw.decoders.for_profile(s, *p, probe);
    let buffer = dump::Captured::wrap(dma, pending.map(dump::Pending::trace));
    submit_profile(pipe, (buffer, fb, avail), (id, completed.clock), (newest, tap), callbacks, out);
    true
}
