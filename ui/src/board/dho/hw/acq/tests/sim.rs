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

    pub(super) fn seq(&self) -> u64 {
        self.seq
    }
}

#[derive(Debug, PartialEq)]
pub(super) enum Event {
    Write(u32, u32),
    Read(u32, u32),
    ReadError(u32),
    Dma(usize),
    DmaTimeout(Duration),
    Wait(Duration),
    Park(Duration),
    ParkTimeout(Duration),
    Intake(Submission),
    Picture(Picture),
    Stop,
    Run,
    SourceWake,
    SettingsPublished,
    SettingsApplied,
    RingClear(oos_core::change::Identity),
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

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum DoneRead {
    #[default]
    Normal,
    Cleared,
    AllOnes,
    Error,
}

#[derive(Default)]
struct StopSignals {
    elapsed: Cell<Duration>,
    pulse_at: Cell<Option<Duration>>,
    delay: Cell<Duration>,
    hide_stopped: Cell<bool>,
    done: Cell<DoneRead>,
}

pub(super) struct SimBus {
    engine: sim::SimBus,
    trace: Rc<RefCell<Trace>>,
    stop: Rc<StopSignals>,
}

impl Bus for SimBus {
    fn wr(&mut self, reg: u32, value: u32) {
        self.trace.borrow_mut().events.push(Event::Write(reg, value));
        let armed = self.engine.0.borrow().capture_armed();
        self.engine.wr(reg, value);
        if reg == 0x4004 && value >> 31 != 0 && armed {
            self.stop.pulse_at.set(Some(self.stop.elapsed.get()));
        } else if !armed && self.engine.0.borrow().capture_armed() {
            self.stop.pulse_at.set(None);
        }
    }

    fn rd(&mut self, reg: u32) -> u32 {
        use oos_hal::dho::scu::{status::WAIT_TRIG, PLAYINFO_DONE};

        let mut value = match (reg, self.stop.pulse_at.get(), self.stop.done.get()) {
            (0x4080, Some(_), DoneRead::AllOnes) => u32::MAX,
            (0x4080, Some(_), DoneRead::Cleared) => 0,
            args => match (args, self.engine.rd(reg)) {
                // The canceled first-trigger wait captured on the unit had one completed record.
                ((0x4034, None, DoneRead::Cleared), flags) if flags == WAIT_TRIG | 0x20 => 0x0cd2,
                ((0x4080, None, DoneRead::Cleared), PLAYINFO_DONE) => 0x4000_0010,
                ((0x4034, Some(_), DoneRead::Cleared), _) => 0x20c0,
                ((0x4034, _, _), WAIT_TRIG) => 0x12,
                (_, value) => value,
            },
        };
        let late = self.stop.pulse_at.get().is_some_and(|at| self.stop.elapsed.get() - at < self.stop.delay.get());
        if reg == 0x4034 && (self.stop.hide_stopped.get() || late) {
            value &= !oos_hal::dho::scu::status::STOPPED;
        }
        self.trace.borrow_mut().events.push(Event::Read(reg, value));
        value
    }

    fn try_rd(&mut self, reg: u32) -> io::Result<u32> {
        if reg == 0x4080 && self.stop.pulse_at.get().is_some() && self.stop.done.get() == DoneRead::Error {
            self.trace.borrow_mut().events.push(Event::ReadError(reg));
            return Err(io::Error::other("simulated play-info failure"));
        }
        Ok(self.rd(reg))
    }
}

pub(super) struct SimStream {
    stream: sim::SimC2h,
    trace: Rc<RefCell<Trace>>,
    edge_at: Option<usize>,
    run: Run,
    stop: Rc<StopSignals>,
    on_dma: Option<DmaHook>,
    read_error: Option<io::ErrorKind>,
}

type DmaHook = Box<dyn FnOnce(&mut Trace)>;

impl SimStream {
    pub(super) fn on_dma(&mut self, hook: impl FnOnce(&mut Trace) + 'static) {
        self.on_dma = Some(Box::new(hook));
    }

    pub(super) fn fail_next(&mut self) {
        self.read_error = Some(io::ErrorKind::Other);
    }

    pub(super) fn timeout_next(&mut self) {
        self.read_error = Some(io::ErrorKind::TimedOut);
    }
}

impl Read for SimStream {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        let count = self.stream.read(bytes)?;
        if let Some(kind) = self.read_error.take() {
            // The transfer consumed the data before reporting its error.
            if kind == io::ErrorKind::TimedOut {
                let timeout = Duration::from_secs(1);
                self.stop.elapsed.set(self.stop.elapsed.get() + timeout);
                self.trace.borrow_mut().events.push(Event::DmaTimeout(timeout));
            }
            return Err(kind.into());
        }
        if let Some(hook) = self.on_dma.take() {
            hook(&mut self.trace.borrow_mut());
        }
        self.trace.borrow_mut().events.push(Event::Dma(count));
        let mark_first_sample =
            matches!(self.run, Run::Live(_) | Run::CompressedTap) || self.stop.done.get() == DoneRead::Cleared;
        let mark_entire_record = matches!(self.run, Run::Pictures(_));
        if self.edge_at.is_some() || mark_first_sample || mark_entire_record {
            let engine = self.stream.0.borrow();
            let ratio = engine.ratio as usize;
            let words = engine.regs[&0x11f0] as usize * ratio;
            let prior = engine.triggers - u64::from(engine.recorded);
            for record in bytes[..count].chunks_exact_mut(words * 2) {
                if let Some(pre) = self.edge_at.filter(|_| record[..2] == sim::Engine::word(1, 0).to_le_bytes()) {
                    // Preserve sample zero's record number and give frame one a rising edge at pre.
                    for (i, word) in record.as_chunks_mut::<2>().0.iter_mut().enumerate().skip(1) {
                        let code: u16 = if i / ratio < pre { 25_000 } else { 40_000 };
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
    engine: Engine,
    out: HwShared,
    trace: Rc<RefCell<Trace>>,
    run: Run,
    intake: Option<Intake>,
    timed_out: Cell<bool>,
    stop_fill: Cell<Option<u32>>,
    driver: RefCell<Option<Driver>>,
    signals: Rc<StopSignals>,
}

impl Clock for Virtual {
    fn now(&self) -> Instant {
        self.origin + self.signals.elapsed.get()
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
        assert!(self.signals.elapsed.get() < Duration::from_secs(30), "script did not reach its completion event");
        trace.events.push(event);
        self.signals.elapsed.set(self.signals.elapsed.get() + duration);
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

            fn try_rd(&mut self, reg: u32) -> io::Result<u32> {
                self.0.try_rd(reg)
            }
        }
        Borrowed(bus)
    }

    fn apply_settings(io: &mut Io<Self>, steps: &[seq::Step]) -> Result<(), String> {
        io.execute(steps)?;
        let mut trace = io.clock.trace.borrow_mut();
        if trace.events.contains(&Event::SettingsPublished) {
            trace.events.push(Event::SettingsApplied);
        }
        Ok(())
    }

    fn calibrate(io: &mut Io<Self>, _: &str, out: &HwShared, req: service::CalRequest) -> bool {
        assert!(lock(out).hold, "calibration must be dispatched while held");
        io.clock.mark(Event::Calibration(req));
        false
    }

    fn generator(_: &mut service::Service, bar: &mut SimBus, _: &HwShared, want: Option<Gen>) {
        bar.trace.borrow_mut().events.push(Event::Generator(want));
    }

    fn probe_la() -> Option<Option<bool>> {
        None
    }

    fn trim(io: &mut Io<Self>, mask: u32) -> bool {
        io.clock.mark(Event::Trim(mask));
        true
    }

    fn stream_cleared(io: &Io<Self>, identity: oos_core::change::Identity) {
        io.clock.mark(Event::RingClear(identity));
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
    let ratio = oos_hal::dho::hz::ratio(settings.mask);
    let (engine, bus, stream) = sim::engine(sim::Engine::new(10_000_000_000, 1_000_000_000, per_poll, ratio));
    let trace = Rc::new(RefCell::new(Trace::default()));
    let signals = Rc::new(StopSignals::default());
    let state = seq::State::handover(settings.mask, None);
    let edge_at = on_trigger.then(|| {
        let (_, params) = seq::State::handover(settings.mask, None).configure(settings, None, None).unwrap();
        params.pre as usize
    });
    let io = Io {
        bar: SimBus { engine: bus, trace: trace.clone(), stop: signals.clone() },
        c2h: SimStream {
            stream,
            trace: trace.clone(),
            edge_at,
            run,
            stop: signals.clone(),
            on_dma: None,
            read_error: None,
        },
        clock: Virtual {
            origin: Instant::now(),
            engine,
            out,
            trace: trace.clone(),
            run,
            intake,
            timed_out: Cell::default(),
            stop_fill: Cell::new(matches!(run, Run::StopDuring).then_some(1)),
            driver: RefCell::default(),
            signals,
        },
        port: Sim,
        cal: None,
        cal_adc: None,
        dma_fault: Default::default(),
        trim_overlay: Default::default(),
        la_cal: Default::default(),
        state,
    };
    (io, trace)
}
