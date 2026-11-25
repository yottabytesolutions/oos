// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::dho::bus::{DmaBuf, Recorder};
use crate::dho::scu;
use crate::dho::seq::{frame_bytes, wait::STOP_POLL};
use std::cell::Cell;
use time::Clock;

struct TestClock {
    start: Instant,
    elapsed: Cell<Duration>,
}

impl time::Clock for TestClock {
    fn now(&self) -> Instant {
        self.start + self.elapsed.get()
    }

    fn wait(&self, duration: Duration) {
        self.elapsed.set(self.elapsed.get() + duration);
    }
}

#[test]
fn invalid_batch_readout_is_reported_before_any_register_write() {
    let clock = TestClock { start: Instant::now(), elapsed: Cell::default() };
    let mut state = State::handover(1, None);
    let mut settings = Settings { depth: 1000, ..Settings::default() };
    let (_, params) = state.configure(&settings, None, None).unwrap();
    settings.compress = 2;
    settings.window = Some((24, 1 - i64::from(params.pre)));
    let mut bar = Recorder::default();
    let mut dma = DmaBuf::new();
    let error = acquire_batch_with_clock(
        &mut state,
        &settings,
        &params,
        &mut bar,
        &mut std::io::empty(),
        &mut dma,
        false,
        0,
        |_| {},
        || false,
        || false,
        &clock,
    )
    .unwrap_err();
    assert_eq!(error, super::super::ReadoutError::UnalignedWindow { start: 1, points: 24 }.to_string());
    assert!(bar.writes.is_empty());
}

#[test]
fn phase_observer_leaves_the_acquisition_sequence_and_read_unchanged() {
    let settings = Settings { frames: 2, ..Settings::default() };
    let start = Instant::now();
    let run = |observe| {
        let clock = TestClock { start, elapsed: Cell::default() };
        let mut state = State::handover(1, None);
        let (_, params) = state.configure(&settings, None, None).unwrap();
        let mut bar = Recorder::default();
        bar.regs.insert(0x4080, scu::PLAYINFO_DONE);
        let mut dma = DmaBuf::new();
        let mut logs = Vec::new();
        let count = acquire_batch_phases(
            &mut state,
            &settings,
            &params,
            &mut bar,
            &mut std::io::repeat(7),
            &mut dma,
            false,
            BatchWait::new(1, None),
            |message| logs.push(message),
            || false,
            || false,
            &clock,
            observe,
        )
        .unwrap();
        (count, dma.data().to_vec(), bar.writes, logs)
    };

    let mut phases = Vec::new();
    let observed = run(Some((|phase| phases.push(phase), || false, (|_| true) as fn(u32) -> bool)));
    let plain = run(None);
    assert_eq!(observed, plain);
    assert!(matches!(phases.as_slice(), [
        BatchPhase::Arm(_),
        BatchPhase::FirstTrigger(wait),
        BatchPhase::FillStarted(at),
        BatchPhase::Fill(elapsed),
        BatchPhase::Readout(_),
        BatchPhase::Read(_, bytes),
    ] if wait.is_zero() && *at == start && *elapsed == Duration::from_millis(1) && *bytes >= observed.1.len()));
}

struct TimedBus<'a> {
    clock: &'a TestClock,
    first: Duration,
    extra: Duration,
    polls: Vec<(u32, Duration)>,
    writes: Vec<(u32, u32)>,
}

impl crate::dho::bus::Bus for TimedBus<'_> {
    fn wr(&mut self, register: u32, value: u32) {
        self.writes.push((register, value));
    }

    fn rd(&mut self, register: u32) -> u32 {
        let elapsed = self.clock.elapsed.get();
        self.polls.push((register, elapsed));
        match register {
            0x4080 if elapsed >= self.first => scu::PLAYINFO_DONE,
            0x4010 if elapsed >= self.first + self.extra => 2,
            _ => 0,
        }
    }
}

fn timed_bus(clock: &TestClock, first_ms: u64, extra_ms: u64) -> TimedBus<'_> {
    TimedBus {
        clock,
        first: Duration::from_millis(first_ms),
        extra: Duration::from_millis(extra_ms),
        polls: Vec::new(),
        writes: Vec::new(),
    }
}

#[test]
fn estimated_wait_is_recomputed_until_the_count_reaches_the_plan() {
    let clock = TestClock { start: Instant::now(), elapsed: Cell::default() };
    let settings = Settings { frames: 3, ..Settings::default() };
    let mut state = State::handover(1, None);
    let params =
        hz::Params { memsel: 4_000, pre: 2_000, rate: 1_000_000, max_frames: 3, ..hz::prep(1, 1_000_000_000, 4096, 0) };
    let mut bar = timed_bus(&clock, 2, 15);
    let mut dma = DmaBuf::new();
    let mut phases = Vec::new();
    let count = acquire_batch_phases(
        &mut state,
        &settings,
        &params,
        &mut bar,
        &mut std::io::repeat(1),
        &mut dma,
        false,
        BatchWait::new(30, None),
        |_| {},
        || false,
        || false,
        &clock,
        Some((|phase| phases.push(phase), || false, |_| true)),
    )
    .unwrap();
    assert_eq!(count, 3);
    let us = Duration::from_micros;
    assert_eq!(
        bar.polls.iter().filter(|(reg, _)| *reg == 0x4010).map(|(_, at)| *at).collect::<Vec<_>>(),
        [us(2000), us(2000), us(10_000), us(12_000), us(20_000)]
    );
    assert!(matches!(phases[1], BatchPhase::FirstTrigger(wait) if wait == us(2000)));
    assert!(matches!(phases[3], BatchPhase::Fill(wait) if wait == us(18_000)));
}

#[test]
fn serial_capture_can_be_canceled_at_admission_without_a_dma_read() {
    let clock = TestClock { start: Instant::now(), elapsed: Cell::default() };
    let settings = Settings { frames: 1, ..Settings::default() };
    let mut state = State::handover(1, None);
    let params = hz::Params { memsel: 4_000, pre: 2_000, rate: 1_000_000, ..hz::prep(1, 1_000_000_000, 4096, 0) };
    let mut bar = timed_bus(&clock, 2, 0);
    let mut dma = DmaBuf::new();
    let attempted = Cell::new(0);
    let result = acquire_batch_phases(
        &mut state,
        &settings,
        &params,
        &mut bar,
        &mut std::io::Cursor::new([]),
        &mut dma,
        false,
        BatchWait::new(30, None),
        |_| {},
        || false,
        || false,
        &clock,
        Some((
            |_| {},
            || false,
            |captured| {
                assert_eq!(clock.elapsed.get(), Duration::from_millis(2));
                attempted.set(captured);
                false
            },
        )),
    );
    assert_eq!(result, Err(NEW_SETTINGS.into()));
    assert_eq!(attempted.get(), 1);
    assert!(dma.data().is_empty());
}

#[test]
fn initial_timed_wait_keeps_the_ten_millisecond_stop_bound() {
    let clock = TestClock { start: Instant::now(), elapsed: Cell::default() };
    let mut bar = timed_bus(&clock, 1000, 0);
    let poll = Step::Poll { reg: 0x4080, bit: scu::PLAYINFO_DONE, ms: 2000, what: "test" };
    let stop = || !clock.elapsed.get().is_zero();
    assert_eq!(
        poll_first(&poll, &mut bar, &mut |_| {}, &stop, &clock, clock.start + Duration::from_millis(100)),
        Err(NEW_SETTINGS.to_string())
    );
    assert_eq!(clock.elapsed.get(), Duration::from_millis(10));
}

#[test]
fn fill_checks_controls_at_ten_milliseconds_with_short_records() {
    let clock = TestClock { start: Instant::now(), elapsed: Cell::default() };
    let mut bar = timed_bus(&clock, 0, 1000);
    let params = hz::prep(1, 1_000_000_000, 4096, 0);
    let polls = Cell::new(0);
    let stop = || {
        polls.set(polls.get() + 1);
        true
    };
    assert_eq!(
        fill_count(
            &mut bar,
            (3, &params),
            (BatchWait::new(30, None), clock.start, true),
            &clock,
            (&mut || false, &stop)
        ),
        1
    );
    assert_eq!(polls.get(), 1);
    assert_eq!(clock.elapsed.get(), STOP_POLL);
    assert_eq!(bar.polls[1].1, Duration::from_micros(200));
}

#[test]
fn overlapping_fill_reads_after_a_control_request_and_keeps_the_trigger_clock() {
    let clock = TestClock { start: Instant::now(), elapsed: Cell::default() };
    let settings = Settings { frames: 3, ..Settings::default() };
    let params =
        hz::Params { memsel: 4_000, pre: 2_000, rate: 1_000_000, max_frames: 3, ..hz::prep(1, 1_000_000_000, 4096, 0) };
    let mut bar = timed_bus(&clock, 2, 15);
    let mut dma = DmaBuf::new();
    let mut phases = Vec::new();
    let control = Cell::new(false);
    let count = acquire_batch_phases(
        &mut State::handover(1, None),
        &settings,
        &params,
        &mut bar,
        &mut std::io::repeat(2),
        &mut dma,
        true,
        BatchWait::new(40, Some(clock.start)),
        |_| {},
        || control.get(),
        || {
            control.set(true);
            false
        },
        &clock,
        Some((
            |phase| phases.push(phase),
            || false,
            |count| {
                assert_eq!(count, 3);
                clock.wait(Duration::from_millis(20));
                true
            },
        )),
    )
    .unwrap();
    assert_eq!(count, 3);
    assert_eq!(dma.data().len(), 3 * frame_bytes(&settings, &params));
    assert!(matches!(phases[1], BatchPhase::FillStarted(at) if at == clock.start + Duration::from_millis(2)));
    assert!(control.get());
    assert!(matches!(phases[2], BatchPhase::Fill(elapsed) if elapsed == Duration::from_millis(18)));
    assert_eq!(clock.elapsed.get(), Duration::from_millis(40));
}
