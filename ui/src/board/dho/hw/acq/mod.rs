// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! Acquisition thread and its worker-owned state.

use super::super::{
    clock::LoopClock,
    cores::Cores,
    publication::{self, Request},
    replay,
};
use super::{
    comparator::UnitGuard,
    dma_source::{DmaSource, Successor},
    lifecycle,
    segmented::{seg_abort, SegRun, SegmentIo},
    Settings,
};
use crate::{
    board::{counters::Outcome, identity::CaptureStatus, HwShared, Slow, Spawn},
    sysconf::Switches,
};
use oos_core::change::Identity;
use oos_hal::dho::{
    adc, afe, avg,
    bus::{Bus, DmaBuf},
    hz, la,
    seq::{
        self,
        time::{Clock, ExecuteError},
    },
};
use std::{
    io::Read,
    time::{Duration, Instant},
};

mod batch;
mod control;
mod dispatch;
mod dma_fault;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::spawn;
mod pacing;
mod read;
mod reference;
#[cfg(test)]
pub(super) use reference::fixture_profile;
mod roll;
mod service;
#[cfg(test)]
mod tests;
mod units;

use roll::Stream;
use service::Service;
use units::Units;

trait Port: Sized {
    type Bus: Bus;
    type Stream: Read;
    type Clock: LoopClock;

    fn batch_bus<'a>(&self, bus: &'a mut Self::Bus) -> impl Bus + 'a;
    fn apply_settings(io: &mut Io<Self>, steps: &[seq::Step]) -> Result<(), String>;
    fn calibrate(io: &mut Io<Self>, cal_dir: &str, out: &HwShared, req: service::CalRequest) -> bool;
    fn generator(service: &mut Service, bar: &mut Self::Bus, out: &HwShared, want: Option<crate::board::Gen>);
    fn probe_la() -> Option<Option<bool>>;
    fn trim(io: &mut Io<Self>, mask: u32) -> bool;
    #[cfg(test)]
    fn stream_cleared(_io: &Io<Self>, _identity: Identity) {}
}

struct Io<P: Port> {
    bar: P::Bus,
    c2h: P::Stream,
    port: P,
    clock: P::Clock,
    cal: Option<afe::Cal>,
    cal_adc: Option<adc::CalAdc>,
    trim_overlay: oos_hal::dho::coretrim::Overlay,
    la_cal: la::Cal,
    state: seq::State,
    dma_fault: dma_fault::DmaFault,
}

#[derive(Default)]
pub(super) struct Replay {
    fresh: bool,
    last_batch: Option<replay::Batch>,
    replayed: Option<crate::board::identity::StopView>,
    paused: bool,
}

impl Replay {
    fn discard(&mut self) {
        (self.last_batch, self.replayed, self.fresh) = (None, None, false);
    }

    pub(super) fn invalidate(&mut self, out: &HwShared, count: u32) {
        // Every arm starts at slot zero and can write its full programmed range.
        if self.last_batch.as_ref().is_some_and(|batch| batch.index < count) {
            crate::drawpipe::counters().acquisition.disarms().drop_replay();
            self.discard();
            let mut g = crate::perfguard::deep::acq_lock(out);
            let sequence = g.stop_view.map_or(g.frame.seq, |view| g.frame.seq.max(view.capture));
            g.stop_replies.lost_through(sequence);
        }
    }

    pub(super) fn successor_slots(&self, settings: &Settings) -> u32 {
        // Prearm uses the configured geometry of the batch it follows.
        self.last_batch.as_ref().map_or(0, |batch| seq::frames(settings, &batch.params))
    }
}

#[derive(Default)]
struct Configured {
    current: Option<(Settings, hz::Params)>,
    play_window: Option<(usize, Option<(u32, i64)>)>,
    mode: Slow,
    stream_pair: Option<(u64, u64)>,
    unit_blocked: bool,
}

#[derive(Default)]
struct BatchState {
    armed: Successor,
    dma: DmaSource<DmaBuf>,
    raw: Raw,
    guard: UnitGuard,
    la_fails: u32,
    fill: crate::pacing::Fill,
    cap: crate::pacing::FrameCap,
    avg: Option<(avg::Tracker, u64, u32)>,
    prearm_timing: Option<(Instant, Duration)>,
    single_done: Option<SingleCompletion>,
    first: FirstRecord,
    record_wait: publication::RecordWait,
}

struct SingleCompletion {
    pair: (u64, u64),
    handoff: SingleHandoff,
}

#[derive(Clone, Copy)]
enum SingleHandoff {
    AwaitingUi(Instant),
    TimedOut,
    TakenOver,
}

#[derive(Default)]
struct FirstRecord {
    settings: Option<Settings>,
    dropped: bool,
    proven: Option<(u64, u64)>,
}

#[derive(Default)]
struct Raw {
    dma: DmaBuf,
    cores: Cores,
    live: Option<Instant>,
    deep: Option<Instant>,
    dump: Option<super::header::dump::Dump>,
    reference: super::raw::ReferenceCycle,
    decoders: reference::Decoders,
}

struct AcqLoop<T: pacing::PacingPolicy> {
    options: Spawn,
    switches: Switches,
    out: HwShared,
    configured: Configured,
    seq: u64,
    batch: BatchState,
    replay: Replay,
    stream: Stream,
    service: Service,
    units: Units,
    segrun: Option<SegRun>,
    pacing: T,
}

impl<T: pacing::PacingPolicy> AcqLoop<T> {
    fn new(
        options: Spawn,
        switches: Switches,
        out: HwShared,
        service: Service,
        dump: Option<super::header::dump::Dump>,
        now: Instant,
        pacing: T,
    ) -> Self {
        let stream = Stream::new(now);
        Self {
            options,
            switches,
            out,
            service,
            configured: Configured::default(),
            seq: 0,
            batch: BatchState { raw: Raw { dump, ..Raw::default() }, ..BatchState::default() },
            replay: Replay::default(),
            stream,
            units: Units::default(),
            segrun: None,
            pacing,
        }
    }

    fn run(&mut self, io: &mut Io<impl Port>) {
        while let Some(counter) = Service::before_pass(&self.out) {
            let request = self.service(io, counter);
            if io.dma_fault.pending() {
                self.replay.invalidate(&self.out, u32::MAX);
                self.batch.first = FirstRecord::default();
                {
                    let mut g = crate::perfguard::deep::acq_lock(&self.out);
                    if g.hold {
                        // The timed-out play invalidated the ring. There is no final DMA to publish.
                        g.final_ack = g.final_req;
                    }
                }
                if !io.dma_fault.recover(&mut io.state, &mut io.bar, &io.clock, &self.out) {
                    continue;
                }
                self.configured.current = None;
            }
            let mut last_read = std::mem::take(&mut self.replay.fresh);
            let Some(request) = request else { continue };
            let Some((settings, params)) = self.reconfigure(io, &request, &mut last_read) else { continue };
            if self.configured.mode != Slow::Off {
                if !self.stream.poll(
                    io,
                    &self.out,
                    (&settings, &params, self.configured.mode),
                    (&mut self.seq, request.identity),
                    |io| io.disarm(&mut self.batch, &mut self.replay, &self.out),
                ) {
                    self.configured.current = None;
                }
                continue;
            }
            self.stream.stop(io);
            if settings.segments.is_some() {
                self.segments(io, &settings, &params, request.identity);
            } else {
                self.batch(io, (&settings, &params), request, last_read);
            }
        }
    }

    fn segments(&mut self, io: &mut Io<impl Port>, settings: &Settings, params: &hz::Params, identity: Identity) {
        io.disarm(&mut self.batch, &mut self.replay, &self.out);
        if !self.batch.dma.ready(identity, &io.clock, || {
            let g = crate::perfguard::deep::acq_lock(&self.out);
            publication::boundary(Some((identity.gen(), identity.epoch())), g.acquisition.identity())
                || g.hold
                || g.cal_req.is_some()
        }) {
            return;
        }
        let mut segment = SegmentIo::new(
            &mut io.state,
            &mut io.bar,
            &mut io.c2h,
            self.batch.dma.destination(),
            &self.out,
            &mut self.seq,
        );
        if let Err(e) = segment.step(&mut self.segrun, settings, params, identity) {
            // The legacy segment executor reports its C2H timeouts as text.
            let error = if e.contains(seq::time::DMA_TIMEOUT) { ExecuteError::DmaTimeout(e) } else { e.into() };
            io.report_read_error(&self.out, "segments", error);
            io.abort_segments(&mut self.segrun, &self.out);
            if !io.dma_fault.pending() {
                io.clock.wait(Duration::from_millis(200));
            }
        }
    }
}

impl<P: Port> Io<P> {
    pub(super) fn report_read_error(&mut self, out: &HwShared, context: &str, error: ExecuteError) {
        self.dma_fault.report(&mut self.state, &mut self.bar, &self.clock, out, context, error);
    }

    fn execute(&mut self, steps: &[seq::Step]) -> Result<Vec<u8>, ExecuteError> {
        execute(steps, &mut self.bar, &mut self.c2h, &self.clock)
    }

    fn disarm(&mut self, batch: &mut BatchState, replay: &mut Replay, out: &HwShared) {
        batch.first.proven = None;
        batch.armed.disarm(replay, out, || disarm(&mut self.state, &mut self.bar, &self.clock, out));
    }

    fn abort_segments(&mut self, run: &mut Option<SegRun>, out: &HwShared) {
        seg_abort(run, &mut self.state, &mut self.bar, &mut self.c2h, out);
    }
}

fn execute(
    steps: &[seq::Step],
    bar: &mut impl Bus,
    c2h: &mut impl Read,
    clock: &impl Clock,
) -> Result<Vec<u8>, ExecuteError> {
    let mut dma = DmaBuf::new();
    seq::time::execute_bus_with_clock(steps, bar, c2h, &mut dma, &mut |_| {}, &|| false, clock)?;
    Ok(dma.into_vec())
}

fn disarm(state: &mut seq::State, bar: &mut impl Bus, clock: &impl Clock, out: &HwShared) {
    let report = lifecycle::disarm(|| state.disarm_capture(bar, clock));
    let outcome = match report.outcome() {
        seq::CaptureStop::Unknown => Outcome::Unknown,
        seq::CaptureStop::NoCompletedRecord => Outcome::NoCompletedRecord,
        seq::CaptureStop::Recorded => Outcome::Recorded,
    };
    crate::drawpipe::counters().acquisition.disarms().observe(outcome);
    let stop_status = matches!(outcome, Outcome::Unknown)
        .then(|| {
            let mut g = crate::perfguard::deep::acq_lock(out);
            let publication = g.acquisition;
            g.hold.then(|| g.status.report(publication, CaptureStatus::StopUnconfirmed, report.error()))
        })
        .flatten();
    if let Some(message) = stop_status {
        eprintln!("oos-ui hw: {message}");
    } else if let Some(error) = report.error() {
        status(out, format!("disarm: {error}"));
    }
}

fn status(out: &HwShared, message: String) {
    eprintln!("oos-ui hw: {}", message);
    crate::perfguard::deep::acq_lock(out).status.set(message);
}
