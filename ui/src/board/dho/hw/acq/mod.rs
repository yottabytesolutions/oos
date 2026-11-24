// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! Acquisition thread and its worker-owned state.

use super::super::{
    cores::Cores,
    publication::{self, Request},
    replay,
};
use super::{
    comparator::UnitGuard,
    dma_source::{ArmedBatch, DmaSource},
    lifecycle, overlay,
    segmented::{seg_abort, SegRun, SegmentIo},
    startup, Settings,
};
use crate::{
    board::{HwShared, Shared, Slow, Spawn},
    sysconf::Switches,
};
use oos_core::change::Identity;
use oos_hal::dho::{
    adc, afe, avg,
    bus::{Bar1, DmaBuf},
    hz, la, seq,
};
use std::{
    fs::File,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

mod batch;
mod control;
mod dispatch;
mod read;
mod roll;
mod service;
mod units;

use roll::Stream;
use service::Service;
use units::Units;

struct Io {
    bar: Bar1,
    c2h: File,
    afe_bus: Option<afe::GpioAfe>,
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

struct AcqLoop {
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
}

impl AcqLoop {
    fn new(options: Spawn, switches: Switches, out: HwShared) -> Self {
        let stream = Stream::default();
        let service = Service::new(&options.cal_dir);
        let dump = super::header::dump::Dump::from_switch(&switches.board.batch_dump);
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
        }
    }

    fn run(&mut self, io: &mut Io) {
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

    fn segments(&mut self, io: &mut Io, settings: &Settings, params: &hz::Params, identity: Identity) {
        io.disarm(&mut self.batch.armed);
        if !self.batch.dma.ready(identity, || {
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

impl Io {
    fn open(o: &Spawn, switches: &Switches, out: &HwShared) -> Result<Self, String> {
        let (bar, c2h) = startup::open_acquisition(switches, out)?;
        let afe_bus =
            if o.program_afe { Some(afe::GpioAfe::open().map_err(|e| format!("AFE GPIO: {}", e))?) } else { None };
        let cal = match afe::Cal::load_from(&o.cal_dir) {
            Ok((c, from)) => {
                status(
                    out,
                    format!(
                        "calibration: {}",
                        from.iter().map(|(n, d)| format!("{} from {}", n, d)).collect::<Vec<_>>().join(", ")
                    ),
                );
                Some(c)
            }
            Err(_) => None,
        };
        let mut cal_adc = adc::CalAdc::load(&o.cal_dir).ok();
        // the matched core offsets: only with `cal.core_offset = tight` (key off = the cal_adc.hex above, unchanged)
        let trim_overlay = overlay::in_use(switches.board.core_offset_tight, overlay::load);
        if let Some(c) = cal_adc.as_mut() {
            trim_overlay.apply(c);
        }
        // LA pod zero codes (cal_la.hex, read only); without the file the middle of the DAC range (la.md 8.1)
        let la_cal = if oos_hal::model::platform().caps.la {
            oos_hal::dho::la::Cal::load(&o.cal_dir).unwrap_or_else(|| {
                status(out, "cal_la.hex not found: LA thresholds with the zero code 0x2180".into());
                oos_hal::dho::la::Cal { zero: [0x2180; 2] }
            })
        } else {
            oos_hal::dho::la::Cal::default()
        };
        let state = seq::State::handover(o.mask, cal_adc.as_ref());
        Ok(Self { bar, c2h, afe_bus, cal, cal_adc, trim_overlay, la_cal, state })
    }

    fn disarm(&mut self, armed: &mut Option<super::dma_source::ArmedBatch>) {
        if armed.take().is_some() {
            if let Err(e) = lifecycle::disarm(|| seq::disarm(&mut self.state, &mut self.bar, &mut self.c2h, |_| {})) {
                eprintln!("oos-ui hw: disarm: {}", e);
            }
        }
    }

    fn abort_segments(&mut self, run: &mut Option<SegRun>, out: &HwShared) {
        seg_abort(run, &mut self.state, &mut self.bar, &mut self.c2h, out);
    }
}

fn status(out: &HwShared, message: String) {
    eprintln!("oos-ui hw: {}", message);
    crate::perfguard::deep::acq_lock(out).status = message;
}

fn wait_for_startup(path: Option<&std::path::Path>, out: &HwShared) -> Result<(), String> {
    let Some(path) = path else { return Ok(()) };
    status(out, "waiting for the start-up".into());
    loop {
        let v = oos_hal::bootstatus::parse(&std::fs::read_to_string(path).unwrap_or_default());
        match v.finished {
            Some(Ok(())) => return Ok(()),
            Some(Err(e)) => return Err(format!("acquisition hardware not started: {}", e)),
            None => std::thread::sleep(Duration::from_millis(100)),
        }
    }
}

/// Starts the acquisition thread with the hardware left by startup.
pub fn spawn(options: Spawn, switches: Switches) -> HwShared {
    let shared = Arc::new(Mutex::new(Shared::default()));
    let out = shared.clone();
    let spawned = std::thread::Builder::new().name("acq".into()).spawn(move || {
        crate::placement::place_acquisition(&switches);
        let io = wait_for_startup(options.start_up.as_deref().map(std::path::Path::new), &out)
            .and_then(|()| Io::open(&options, &switches, &out));
        match io {
            Ok(mut io) => AcqLoop::new(options, switches, out).run(&mut io),
            Err(error) => status(&out, error),
        }
    });
    if let Err(e) = spawned {
        eprintln!("oos-ui hw: no acquisition thread: {}", e);
    }
    shared
}
