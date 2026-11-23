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
    dma_source::{ArmedBatch, DmaSource},
    lifecycle,
    segmented::{seg_abort, SegRun, SegmentIo},
    Settings,
};
use crate::{
    board::{HwShared, Slow, Spawn},
    sysconf::Switches,
};
use oos_core::change::Identity;
use oos_hal::dho::{
    adc, afe, avg,
    bus::{Bus, DmaBuf},
    hz, la,
    seq::{self, time::Clock},
};
use std::{
    io::Read,
    time::{Duration, Instant},
};

mod batch;
mod control;
mod dispatch;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::spawn;
mod pacing;
mod read;
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
}

#[derive(Default)]
struct Replay {
    fresh: bool,
    last_batch: Option<replay::Batch>,
    replayed: Option<(f64, f64)>,
    paused: bool,
}

#[derive(Default)]
struct Configured {
    current: Option<(Settings, hz::Params)>,
    mode: Slow,
    applied_pair: Option<(u64, u64)>,
    unit_blocked: bool,
}

#[derive(Default)]
struct BatchState {
    armed: Option<ArmedBatch>,
    dma: DmaSource<DmaBuf>,
    raw: Raw,
    guard: UnitGuard,
    la_fails: u32,
    fill: crate::pacing::Fill,
    cap: crate::pacing::FrameCap,
    avg: Option<(avg::Tracker, u64, u32)>,
    prearm_timing: Option<(Instant, Duration)>,
    single_done: Option<(u64, u64)>,
    first: FirstRecord,
    record_wait: publication::RecordWait,
}

#[derive(Default)]
struct FirstRecord {
    settings: Option<Settings>,
    dropped: bool,
}

#[derive(Default)]
struct Raw {
    dma: DmaBuf,
    cores: Cores,
    live: Option<Instant>,
    deep: Option<Instant>,
    dump: Option<super::header::dump::Dump>,
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
            let mut last_read = std::mem::take(&mut self.replay.fresh);
            let Some(request) = self.service(io, counter) else { continue };
            let Some((settings, params)) = self.reconfigure(io, &request, &mut last_read) else { continue };
            let recording = self.segrun.as_ref().is_some_and(SegRun::recording);
            if !recording {
                self.units.poll(
                    io,
                    &mut self.batch.armed,
                    &self.out,
                    (&settings, self.configured.mode),
                    (&request, self.service.generator_on()),
                );
            }
            if self.configured.mode != Slow::Off {
                if !self.stream.poll(
                    io,
                    &mut self.batch.armed,
                    &self.out,
                    (&settings, &params, self.configured.mode),
                    (&mut self.seq, request.identity),
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
        io.disarm(&mut self.batch.armed);
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
            status(&self.out, format!("segments: {}", e));
            io.abort_segments(&mut self.segrun, &self.out);
            std::thread::sleep(Duration::from_millis(200));
        }
    }
}

impl<P: Port> Io<P> {
    fn execute(&mut self, steps: &[seq::Step]) -> Result<Vec<u8>, String> {
        execute(steps, &mut self.bar, &mut self.c2h, &self.clock)
    }

    fn disarm(&mut self, armed: &mut Option<ArmedBatch>) {
        disarm(armed, &mut self.state, &mut self.bar, &mut self.c2h, &self.clock);
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
) -> Result<Vec<u8>, String> {
    let mut dma = DmaBuf::new();
    seq::time::execute_bus_with_clock(steps, bar, c2h, &mut dma, &mut |_| {}, &|| false, clock)?;
    Ok(dma.into_vec())
}

fn disarm(
    armed: &mut Option<ArmedBatch>,
    state: &mut seq::State,
    bar: &mut impl Bus,
    c2h: &mut impl Read,
    clock: &impl Clock,
) {
    if armed.take().is_some() {
        if let Err(e) = lifecycle::disarm(|| execute(&state.disarm_steps(), bar, c2h, clock)) {
            eprintln!("oos-ui hw: disarm: {}", e);
        }
    }
}

fn status(out: &HwShared, message: String) {
    eprintln!("oos-ui hw: {}", message);
    crate::perfguard::deep::acq_lock(out).status = message;
}
