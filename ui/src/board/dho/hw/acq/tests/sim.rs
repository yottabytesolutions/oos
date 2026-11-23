// SPDX-License-Identifier: GPL-2.0-or-later

use super::super::{pacing::PacingPolicy, service, Io, LoopClock, Port, Settings};
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

#[derive(Clone, Copy)]
pub(super) enum Run {
    Selection,
    DepositBackpressure,
    Live(usize),
    StopDuring,
    StopBetween,
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
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Event {
    Write(u32, u32),
    Read(u32, u32),
    Dma(usize),
    Wait(Duration),
    Park(Duration),
    ParkTimeout(Duration),
    Intake(Submission),
    Stop,
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
}

impl Bus for SimBus {
    fn wr(&mut self, reg: u32, value: u32) {
        self.trace.borrow_mut().events.push(Event::Write(reg, value));
        self.engine.wr(reg, value);
    }

    fn rd(&mut self, reg: u32) -> u32 {
        let value = self.engine.rd(reg);
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
        let mark_first_sample = matches!(self.run, Run::Live(_));
        if self.edge_at.is_some() || mark_first_sample {
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
            }
        }
        Ok(count)
    }
}

pub(super) struct Virtual {
    origin: Instant,
    elapsed: Cell<Duration>,
    engine: Engine,
    out: HwShared,
    trace: Rc<RefCell<Trace>>,
    run: Run,
    intake: Option<Intake>,
    timed_out: Cell<bool>,
}

impl Clock for Virtual {
    fn now(&self) -> Instant {
        self.origin + self.elapsed.get()
    }

    fn wait(&self, duration: Duration) {
        self.elapse(duration, Event::Wait(duration));
        if matches!(self.run, Run::StopDuring) {
            let mut engine = self.engine.borrow_mut();
            let arm = engine.regs.get(&0x400c).copied().unwrap_or_default();
            if engine.per_poll == 0 && arm & 0x6200_0000 == 0x2200_0000 {
                self.stop(&mut lock(&self.out));
                engine.per_poll = 1;
            }
        }
        let mut g = lock(&self.out);
        if g.final_ack != 0 {
            g.record_flow.shutdown = true;
        }
    }
}

impl Virtual {
    pub(super) fn take_submission(&self) -> bool {
        let Some(submission) = self.intake.as_ref().and_then(Intake::take) else { return false };
        self.trace.borrow_mut().events.push(Event::Intake(submission));
        true
    }

    fn elapse(&self, duration: Duration, event: Event) {
        let mut trace = self.trace.borrow_mut();
        assert!(trace.events.len() < 4000, "script did not reach its completion event");
        trace.events.push(event);
        self.elapsed.set(self.elapsed.get() + duration);
    }

    fn stop(&self, g: &mut crate::board::Shared) {
        self.trace.borrow_mut().events.push(Event::Stop);
        g.hold = true;
        g.final_req = 7;
        g.record_flow.wake();
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
        let mut g = lock(&self.out);
        match self.run {
            Run::Live(batches) => {
                let mut trace = self.trace.borrow_mut();
                trace.screens.push(g.queue.drain(..).map(|f| Frame::read(&f)).collect());
                g.record_flow.shutdown = trace.screens.len() == batches;
                g.record_flow.wake();
            }
            Run::StopBetween => self.stop(&mut g),
            Run::DepositBackpressure => {
                assert!(self.take_submission(), "there must be a prior batch to release");
                assert_ne!(event.generation(), observed, "releasing the DMA lease must wake the pipe");
                assert!(event.wait(observed, Duration::ZERO).is_none());
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

    fn calibrate(_: &mut Io<Self>, _: &str, _: &HwShared, _: service::CalRequest) -> bool {
        panic!("unexpected self-calibration");
    }

    fn generator(_: &mut service::Service, _: &mut SimBus, _: &HwShared, _: Option<Gen>) {
        panic!("unexpected generator request");
    }

    fn probe_la() -> Option<Option<bool>> {
        None
    }

    fn trim(_: &mut Io<Self>, _: u32) -> bool {
        panic!("unexpected core trim");
    }
}

pub(super) fn fixture(
    settings: &Settings,
    out: HwShared,
    (run, records, on_trigger): (Run, u32, bool),
    intake: Option<Intake>,
) -> (Io<Sim>, Rc<RefCell<Trace>>) {
    let per_poll = if matches!(run, Run::StopDuring) { 0 } else { records };
    let (engine, bus, stream) = sim::engine(sim::Engine::new(10_000_000_000, 1_000_000_000, per_poll, 1));
    let trace = Rc::new(RefCell::new(Trace::default()));
    let state = seq::State::handover(settings.mask, None);
    let edge_at = on_trigger.then(|| {
        let (_, params) = seq::State::handover(settings.mask, None).configure(settings, None, None).unwrap();
        params.pre as usize
    });
    let io = Io {
        bar: SimBus { engine: bus, trace: trace.clone() },
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
