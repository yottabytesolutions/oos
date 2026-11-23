// SPDX-License-Identifier: GPL-2.0-or-later

use super::super::{pacing::PacingPolicy, service, Io, LoopClock, Port, Settings};
use super::picture::{self, Picture, StopAt};
use crate::{
    board::{dho::clock::Wake, Gen, HwShared},
    drawpipe::test_scope::{Intake, Submission},
};
use oos_hal::{
    dho::{
        bus::Bus,
        seg::sim,
        seq::{self, time::Clock},
    },
    sync::lock,
};
use std::{
    cell::{Cell, RefCell},
    io::{self, Read},
    rc::Rc,
    time::{Duration, Instant},
};

mod driver;

#[derive(Clone, Copy)]
pub(super) enum Run {
    Selection,
    DepositBackpressure,
    Live(usize),
    StopDuring,
    StopBetween,
    Pictures(StopAt),
    CompressedTap,
}

impl Run {
    pub(super) fn draws(self) -> bool {
        matches!(self, Self::Pictures(_) | Self::CompressedTap)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Frame {
    seq: u64,
    code: u16,
    average: bool,
}

impl Frame {
    pub(super) fn stamp(seq: u64, record: u16) -> Self {
        Self::live(seq, record, 0)
    }

    pub(super) fn live(seq: u64, record: u16, prior: u16) -> Self {
        Self { seq, code: (record << 8) | prior, average: false }
    }

    pub(super) fn average(seq: u64, code: u16) -> Self {
        Self { seq, code, average: true }
    }

    pub(super) fn read(f: &crate::board::Frame) -> Self {
        let code = (f64::from(f.ch[0][0]) * 7500.0 + 32768.0).round() as u16;
        Self { seq: f.seq, code, average: f.hw_avg }
    }

    pub(super) fn code(&self) -> u16 {
        self.code
    }
}

#[derive(Debug, PartialEq)]
pub(super) enum Event {
    Write(u32, u32),
    Read(u32, u32),
    Dma(usize),
    Wait(Duration),
    Park(Duration),
    ParkTimeout(Duration),
    Intake(Submission),
    Picture(Picture),
    Stop,
    Run,
    SourceWake,
    Calibration(service::CalRequest),
    Generator(Option<Gen>),
    Trim(u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Pause {
    Wait(Duration),
    Park,
}

#[derive(Default)]
pub(super) struct Trace {
    pub(super) events: Vec<Event>,
    pub(super) screens: Vec<Vec<Frame>>,
}

type Engine = Rc<RefCell<sim::Engine>>;

pub(super) struct SimBus {
    engine: sim::SimBus,
    trace: Rc<RefCell<Trace>>,
    hide_stopped: Rc<Cell<bool>>,
}

impl Bus for SimBus {
    fn wr(&mut self, reg: u32, value: u32) {
        self.trace.borrow_mut().events.push(Event::Write(reg, value));
        self.engine.wr(reg, value);
    }

    fn rd(&mut self, reg: u32) -> u32 {
        let mut value = self.engine.rd(reg);
        if self.hide_stopped.get() && reg == 0x4034 {
            value &= !oos_hal::dho::scu::status::STOPPED;
        }
        self.trace.borrow_mut().events.push(Event::Read(reg, value));
        value
    }
}

pub(super) struct SimStream {
    stream: sim::SimC2h,
    trace: Rc<RefCell<Trace>>,
    edge_at: Option<usize>,
    run: Run,
}

impl Read for SimStream {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        let count = self.stream.read(bytes)?;
        self.trace.borrow_mut().events.push(Event::Dma(count));
        let mark_first_sample = matches!(self.run, Run::Live(_) | Run::CompressedTap);
        let mark_entire_record = matches!(self.run, Run::Pictures(_));
        if self.edge_at.is_some() || mark_first_sample || mark_entire_record {
            let engine = self.stream.0.borrow();
            let words = engine.regs[&0x11f0] as usize;
            let prior = engine.triggers - u64::from(engine.recorded);
            for record in bytes[..count].chunks_exact_mut(words * 2) {
                if let Some(pre) = self.edge_at.filter(|_| record[..2] == sim::Engine::word(1, 0).to_le_bytes()) {
                    // Preserve sample zero's record number and give frame one a rising edge at pre.
                    for (i, word) in record.as_chunks_mut::<2>().0.iter_mut().enumerate().skip(1) {
                        let code: u16 = if i < pre { 25_000 } else { 40_000 };
                        *word = code.to_le_bytes();
                    }
                }
                if mark_first_sample {
                    // The engine repeats slot codes at each arm. Mark records with the trigger count before this arm.
                    record[0] = u8::try_from(prior).unwrap();
                }
                if mark_entire_record {
                    let slot = u16::from(record[1]);
                    let code = picture::record_code(prior + u64::from(slot));
                    for word in record.as_chunks_mut::<2>().0 {
                        *word = code.to_le_bytes();
                    }
                }
            }
        }
        Ok(count)
    }
}

type Driver = Box<dyn FnMut(&Virtual, Pause)>;

pub(super) struct Virtual {
    origin: Instant,
    elapsed: Cell<Duration>,
    engine: Engine,
    out: HwShared,
    trace: Rc<RefCell<Trace>>,
    run: Run,
    intake: Option<Intake>,
    timed_out: Cell<bool>,
    stop_fill: Cell<Option<u32>>,
    driver: RefCell<Option<Driver>>,
    hide_stopped: Rc<Cell<bool>>,
}

impl Clock for Virtual {
    fn now(&self) -> Instant {
        self.origin + self.elapsed.get()
    }

    fn wait(&self, duration: Duration) {
        self.elapse(duration, Event::Wait(duration));
        if let Some(drive) = self.driver.borrow_mut().as_mut() {
            drive(self, Pause::Wait(duration));
        }
        if let Some(records) = self.stop_fill.get() {
            let mut engine = self.engine.borrow_mut();
            let arm = engine.regs.get(&0x400c).copied().unwrap_or_default();
            if engine.per_poll == 0 && arm & 0x6200_0000 == 0x2200_0000 {
                self.stop();
                self.stop_fill.set(None);
                engine.per_poll = records;
            }
        }
        if matches!(self.run, Run::StopDuring | Run::StopBetween) {
            let mut g = lock(&self.out);
            if g.final_ack != 0 {
                g.record_flow.shutdown = true;
            }
        }
    }
}

impl Virtual {
    pub(super) fn take_submission(&self) -> bool {
        let Some(submission) = self.intake.as_ref().and_then(Intake::take) else { return false };
        self.trace.borrow_mut().events.push(Event::Intake(submission));
        true
    }

    pub(super) fn compose(&self) {
        self.take_submission();
        let picture = Picture::compose(crate::drawpipe::pipe().unwrap());
        assert!(!self.take_submission(), "compose may wake the worker without releasing another batch");
        self.trace.borrow_mut().events.push(Event::Picture(picture));
    }

    pub(super) fn stop_next_fill(&self, records: u32) {
        self.engine.borrow_mut().per_poll = 0;
        self.stop_fill.set(Some(records));
    }

    pub(super) fn resume(&self, identity: oos_core::change::Identity) {
        self.hold(false);
        crate::drawpipe::pipe().unwrap().set_identity(identity);
        let mut g = lock(&self.out);
        g.acquisition = crate::board::identity::Publication::new(identity, true);
        g.record_flow.wake();
        self.trace.borrow_mut().events.push(Event::Run);
    }

    fn elapse(&self, duration: Duration, event: Event) {
        let mut trace = self.trace.borrow_mut();
        assert!(self.elapsed.get() < Duration::from_secs(30), "script did not reach its completion event");
        trace.events.push(event);
        self.elapsed.set(self.elapsed.get() + duration);
    }

    pub(super) fn stop(&self) {
        self.trace.borrow_mut().events.push(Event::Stop);
        self.hold(true);
    }

    fn hold(&self, hold: bool) {
        let source = crate::wave::Source::Hw { shared: self.out.clone(), seen: 0 };
        source.hw_hold(hold, 7);
    }
}

impl LoopClock for Virtual {
    fn park(&self, event: &impl Wake, observed: u64, timeout: Duration) -> Option<Duration> {
        assert_eq!(event.generation(), observed);
        if matches!(self.run, Run::DepositBackpressure) && !self.timed_out.replace(true) {
            self.elapse(timeout, Event::ParkTimeout(timeout));
            return Some(timeout);
        }
        let tick = crate::pacing::TIMER_PERIOD;
        assert!(tick <= timeout);
        self.elapse(tick, Event::Park(tick));
        if let Some(drive) = self.driver.borrow_mut().as_mut() {
            drive(self, Pause::Park);
            return Some(tick);
        }
        match self.run {
            Run::Live(batches) => {
                let mut g = lock(&self.out);
                let mut trace = self.trace.borrow_mut();
                trace.screens.push(g.queue.drain(..).map(|f| Frame::read(&f)).collect());
                g.record_flow.shutdown = trace.screens.len() == batches;
                g.record_flow.wake();
            }
            Run::StopBetween => self.stop(),
            Run::DepositBackpressure => {
                assert!(self.take_submission(), "there must be a prior batch to release");
                assert_ne!(event.generation(), observed, "releasing the DMA lease must wake the pipe");
                assert!(event.wait(observed, Duration::ZERO).is_none());
            }
            Run::Pictures(_) => {
                self.compose();
                assert_ne!(event.generation(), observed, "worker publication must wake the source");
            }
            _ => panic!("unexpected record admission wait"),
        }
        Some(tick)
    }
}

pub(super) struct FixedPacing;

impl PacingPolicy for FixedPacing {
    fn picture_period(&self) -> Duration {
        crate::pacing::TIMER_PERIOD
    }

    fn fill_ms(&self, fill: &mut crate::pacing::Fill, now: Instant) -> u64 {
        fill.plan_at_period(now, |_| None, 3.0, self.picture_period())
    }
}

pub(super) struct Sim;

impl Port for Sim {
    type Bus = SimBus;
    type Stream = SimStream;
    type Clock = Virtual;

    fn batch_bus<'a>(&self, bus: &'a mut SimBus) -> impl Bus + 'a {
        struct Borrowed<'a>(&'a mut SimBus);
        impl Bus for Borrowed<'_> {
            fn wr(&mut self, reg: u32, value: u32) {
                self.0.wr(reg, value);
            }

            fn rd(&mut self, reg: u32) -> u32 {
                self.0.rd(reg)
            }
        }
        Borrowed(bus)
    }

    fn apply_settings(io: &mut Io<Self>, steps: &[seq::Step]) -> Result<(), String> {
        io.execute(steps).map(|_| ())
    }

    fn calibrate(io: &mut Io<Self>, _: &str, out: &HwShared, req: service::CalRequest) -> bool {
        assert!(lock(out).hold, "calibration must be dispatched while held");
        io.clock.mark(Event::Calibration(req));
        false
    }

    fn generator(_: &mut service::Service, bar: &mut SimBus, out: &HwShared, want: Option<Gen>) {
        assert!(lock(out).hold, "generator must be dispatched while held");
        bar.trace.borrow_mut().events.push(Event::Generator(want));
    }

    fn probe_la() -> Option<Option<bool>> {
        None
    }

    fn trim(io: &mut Io<Self>, mask: u32) -> bool {
        io.clock.mark(Event::Trim(mask));
        true
    }
}

pub(super) type Fixture = (Io<Sim>, Rc<RefCell<Trace>>);

pub(super) fn fixture(
    settings: &Settings,
    out: HwShared,
    (run, records, on_trigger): (Run, u32, bool),
    intake: Option<Intake>,
) -> Fixture {
    let per_poll = if matches!(run, Run::StopDuring) { 0 } else { records };
    let (engine, bus, stream) = sim::engine(sim::Engine::new(10_000_000_000, 1_000_000_000, per_poll, 1));
    let trace = Rc::new(RefCell::new(Trace::default()));
    let hide_stopped = Rc::new(Cell::new(false));
    let state = seq::State::handover(settings.mask, None);
    let edge_at = on_trigger.then(|| {
        let (_, params) = seq::State::handover(settings.mask, None).configure(settings, None, None).unwrap();
        params.pre as usize
    });
    let io = Io {
        bar: SimBus { engine: bus, trace: trace.clone(), hide_stopped: hide_stopped.clone() },
        c2h: SimStream { stream, trace: trace.clone(), edge_at, run },
        clock: Virtual {
            origin: Instant::now(),
            elapsed: Cell::default(),
            engine,
            out,
            trace: trace.clone(),
            run,
            intake,
            timed_out: Cell::default(),
            stop_fill: Cell::new(matches!(run, Run::StopDuring).then_some(1)),
            driver: RefCell::default(),
            hide_stopped,
        },
        port: Sim,
        cal: None,
        cal_adc: None,
        trim_overlay: Default::default(),
        la_cal: Default::default(),
        state,
    };
    (io, trace)
}
