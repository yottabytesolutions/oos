// SPDX-License-Identifier: GPL-2.0-or-later

use super::{Options, Range, Stage};
use std::{
    cell::{Cell, RefCell},
    hint::black_box,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Default)]
pub(super) struct Work {
    shared: Duration,
    lanes: [Duration; 4],
    invocations: usize,
    calls: usize,
}

impl Work {
    pub(super) fn total(self) -> Duration {
        self.shared + self.lanes.iter().sum::<Duration>()
    }

    pub(super) fn lane(self, index: usize, channels: u32) -> Duration {
        self.lanes[index] + self.shared / channels
    }

    fn add(&mut self, elapsed: Duration, lane: Option<usize>, calls: usize) {
        assert!(self.calls == 0 || self.calls == calls, "each kernel invocation repeats equally");
        self.calls = calls;
        if let Some(lane) = lane {
            self.lanes[lane] += elapsed;
        } else {
            self.shared += elapsed;
        }
        self.invocations += 1;
    }
}

#[derive(Default)]
pub(super) struct Sample {
    stages: [Work; Stage::ALL.len()],
}

impl Sample {
    pub(super) fn work(&self, stage: Stage) -> Work {
        self.stages[stage as usize]
    }

    pub(super) fn calls(&self, stage: Stage) -> usize {
        self.work(stage).calls
    }

    fn add(&mut self, stage: Stage, ns: Duration, lane: Option<usize>, calls: usize) {
        self.stages[stage as usize].add(ns, lane, calls);
    }
}

struct Probe {
    first: usize,
    records: Vec<Sample>,
    options: Options,
}

thread_local! {
    static PROBE: RefCell<Option<Probe>> = const { RefCell::new(None) };
    static RECORD: Cell<Option<usize>> = const { Cell::new(None) };
    static SAMPLING: Cell<bool> = const { Cell::new(false) };
}

pub(super) fn measure(
    range: Range<usize>,
    options: &Options,
    operation: impl FnOnce() -> Duration,
) -> (Vec<Sample>, Duration) {
    PROBE.with_borrow_mut(|slot| {
        assert!(slot.is_none(), "benchmark passes are independent");
        *slot =
            Some(Probe { first: range.start, records: range.map(|_| Sample::default()).collect(), options: *options });
    });
    let reset = operation();
    let probe = PROBE.take().expect("placement left the probe installed");
    (probe.records, reset)
}

pub(crate) fn at<T>(index: usize, operation: impl FnOnce() -> T) -> T {
    if PROBE.with_borrow(Option::is_none) {
        return operation();
    }
    let previous = RECORD.replace(Some(index));
    let output = operation();
    RECORD.set(previous);
    output
}

pub(crate) fn record(index: usize, mut operation: impl FnMut()) {
    let Some(options) = PROBE.with_borrow(|slot| slot.as_ref().map(|probe| probe.options)) else {
        return operation();
    };
    at(index, || {
        let repeats = if options.measures(Stage::Place) { options.repeats() } else { 1 };
        let started = Instant::now();
        for _ in 1..repeats {
            black_box(&mut operation)();
        }
        operation();
        if options.measures(Stage::Place) {
            add(index, Stage::Place, started.elapsed(), None, repeats);
        }
    });
}

fn add(index: usize, stage: Stage, elapsed: Duration, lane: Option<usize>, calls: usize) {
    PROBE.with_borrow_mut(|slot| {
        let probe = slot.as_mut().expect("an active record belongs to a benchmark pass");
        if let Some(record) = index.checked_sub(probe.first).and_then(|i| probe.records.get_mut(i)) {
            record.add(stage, elapsed, lane, calls);
        }
    });
}

/// The inline pool reports active worker time separately from scheduling and sharding.
pub(crate) fn split_dispatch_start() -> Option<Instant> {
    (RECORD.get().is_some()
        && !SAMPLING.get()
        && PROBE.with_borrow(|slot| slot.as_ref().is_some_and(|probe| probe.options.repeats() == 1)))
    .then(Instant::now)
}

pub(crate) fn note_split_dispatch(started: Instant, worker: Duration) {
    if let Some(index) = RECORD.get() {
        add(index, Stage::Split, started.elapsed().saturating_sub(worker), None, 1);
    }
}

pub(crate) fn sample<T>(stage: Stage, operation: impl FnMut() -> T) -> T {
    sample_inner(stage, None, operation)
}

pub(crate) fn sample_channel<T>(stage: Stage, lane: usize, operation: impl FnMut() -> T) -> T {
    sample_inner(stage, Some(lane), operation)
}

fn sample_inner<T>(stage: Stage, lane: Option<usize>, mut operation: impl FnMut() -> T) -> T {
    let Some(index) = RECORD.get().filter(|_| !SAMPLING.get()) else { return operation() };
    let Some(repeats) = PROBE.with_borrow(|slot| {
        slot.as_ref().filter(|probe| probe.options.measures(stage)).map(|probe| probe.options.repeats())
    }) else {
        return operation();
    };
    SAMPLING.set(true);
    let started = Instant::now();
    let mut calls = 0;
    for _ in 1..repeats {
        black_box(black_box(&mut operation)());
        calls += 1;
    }
    let output = operation();
    let elapsed = started.elapsed();
    SAMPLING.set(false);
    add(index, stage, elapsed, lane, calls + 1);
    output
}

pub(crate) fn sample_fold<T>(
    folds: &mut crate::framestats::Folds,
    mut operation: impl FnMut(&mut crate::framestats::Folds) -> T,
) -> T {
    let repeating = PROBE.with_borrow(|slot| {
        slot.as_ref().is_some_and(|probe| probe.options.measures(Stage::Fold) && probe.options.repeats() > 1)
    });
    if folds.is_empty() || !repeating {
        return if folds.is_empty() { operation(folds) } else { sample(Stage::Fold, || operation(folds)) };
    }
    sample(Stage::Fold, || {
        for (_, fold) in folds.iter_mut() {
            *fold = Default::default();
        }
        operation(folds)
    })
}

pub(super) fn account(mut records: Vec<Sample>, reset: Duration, options: &Options) -> Vec<Sample> {
    let count = records.len() as u32;
    for r in &mut records {
        if options.measures(Stage::Reset) {
            r.add(Stage::Reset, reset / count, None, options.repeats());
        }
        if options.repeats() == 1 {
            let attributed = Stage::PLACEMENT.iter().map(|&stage| r.work(stage).total()).sum::<Duration>();
            let place = &mut r.stages[Stage::Place as usize];
            let remainder = place.total().checked_sub(attributed).expect("placement probes measure disjoint intervals");
            r.add(Stage::Remainder, remainder, None, 1);
        }
    }
    records
}

#[test]
fn nested_probes_run_the_kernel_once_per_repeat() {
    let mut inner = 0;
    let options = Options::for_test(Stage::FusedReduceMap, 3);
    let (records, _) = measure(2..3, &options, || {
        record(2, || sample(Stage::FusedReduceMap, || sample(Stage::FusedReduceMap, || inner += 1)));
        Duration::ZERO
    });
    assert_eq!(inner, 3);
    assert_eq!(records[0].work(Stage::FusedReduceMap).invocations, 1);
    assert_eq!(records[0].calls(Stage::FusedReduceMap), 3);
}
