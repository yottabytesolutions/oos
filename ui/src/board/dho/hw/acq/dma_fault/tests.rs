// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::drawpipe::{self, test_scope::Scope};
use oos_hal::{dho::bus::Recorder, sync::lock};
use std::cell::Cell;

struct Virtual(Cell<Instant>);

impl Clock for Virtual {
    fn now(&self) -> Instant {
        self.0.get()
    }

    fn wait(&self, duration: Duration) {
        self.0.set(self.now() + duration);
    }
}

struct Fixture {
    fault: DmaFault,
    state: seq::State,
    bar: Recorder,
    out: HwShared,
    clock: Virtual,
}

impl Fixture {
    fn new() -> Self {
        Self {
            fault: DmaFault::default(),
            state: seq::State::handover(1, None),
            bar: Recorder::default(),
            out: HwShared::default(),
            clock: Virtual(Cell::new(Instant::now())),
        }
    }

    fn report(&mut self, detail: &str) {
        self.fault.report(
            &mut self.state,
            &mut self.bar,
            &self.clock,
            &self.out,
            "batch",
            ExecuteError::DmaTimeout(detail.into()),
        );
    }

    fn fsm(&mut self, flags: u32) {
        self.bar.regs.insert(0x4034, flags);
    }

    fn poll(&mut self) -> bool {
        self.fault.recover(&mut self.state, &mut self.bar, &self.clock, &self.out)
    }

    fn diagnosis(&self) -> Option<CaptureStatus> {
        let g = lock(&self.out);
        g.status.diagnostic(g.acquisition)
    }

    fn disarms(&self) -> usize {
        self.bar.writes.iter().filter(|&&(reg, value)| reg == 0x4004 && value & (1 << 31) != 0).count()
    }
}

#[test]
fn stopped_on_first_poll_releases_without_waiting() {
    let _scope = Scope::new();
    let mut f = Fixture::new();
    f.fsm(scu::status::STOPPED);
    f.report("timed out");
    let before = f.clock.now();
    assert!(f.poll());
    assert!(!f.fault.pending());
    assert_eq!(f.clock.now(), before);
    assert_eq!(f.diagnosis(), None);
    assert_eq!(f.disarms(), 1);
}

#[test]
fn stopped_after_several_polls_releases_without_another_disarm() {
    let _scope = Scope::new();
    let mut f = Fixture::new();
    f.report("timed out");
    for _ in 0..3 {
        assert!(!f.poll());
    }
    f.fsm(scu::status::STOPPED);
    assert!(f.poll());
    assert_eq!(f.diagnosis(), None);
    assert_eq!(f.disarms(), 1);
}

#[test]
fn never_stopped_retries_on_cadence_and_reports_once_at_the_deadline() {
    let _scope = Scope::new();
    let mut f = Fixture::new();
    let start = f.clock.now();
    f.report("timed out");
    assert_eq!(f.disarms(), 1);
    for retry in 1..5 {
        let at = start + RETRY * retry;
        while f.clock.now() < at {
            assert!(!f.poll());
            assert_eq!(f.disarms(), retry as usize);
        }
        assert_eq!(f.clock.now(), at);
        assert!(!f.poll());
        assert_eq!(f.disarms(), retry as usize + 1);
    }
    while f.clock.now() < start + LIMIT {
        assert!(!f.poll());
        assert_eq!(f.diagnosis(), None);
    }
    assert_eq!(f.clock.now(), start + LIMIT);
    assert!(f.poll());
    assert_eq!(f.diagnosis(), Some(CaptureStatus::DmaStopDeadline));
    assert!(lock(&f.out).status.take_latest().contains("0x4034=0x00000000"));
    assert!(f.poll());
    assert!(lock(&f.out).status.latest().is_empty());
}

#[test]
fn all_ones_is_a_link_fault_and_releases_without_waiting() {
    let _scope = Scope::new();
    let mut f = Fixture::new();
    f.report("timed out");
    f.fsm(u32::MAX);
    let before = f.clock.now();
    assert!(f.poll());
    assert_eq!(f.clock.now(), before);
    assert_eq!(f.diagnosis(), Some(CaptureStatus::DmaStopLink));
    assert!(lock(&f.out).status.take_latest().contains("0x4034 returned all ones"));
    assert!(f.poll());
    assert!(lock(&f.out).status.latest().is_empty());
}

#[test]
fn counts_every_timeout_and_limits_the_detailed_log() {
    let _scope = Scope::new();
    let mut f = Fixture::new();
    f.fsm(scu::status::STOPPED);
    f.report("first");
    assert_eq!(lock(&f.out).status.take_latest(), "batch: first");
    assert!(f.poll());
    f.clock.wait(Duration::from_secs(1));
    f.report("second");
    assert!(lock(&f.out).status.take_latest().is_empty());
    assert!(f.poll());
    f.clock.wait(Duration::from_secs(4));
    f.report("third");
    assert_eq!(lock(&f.out).status.take_latest(), "batch: third");
    assert!(drawpipe::counters().acquisition.text().contains(" acq_dma_timeouts=3"));
}
