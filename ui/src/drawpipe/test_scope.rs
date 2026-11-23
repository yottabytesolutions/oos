// SPDX-License-Identifier: GPL-2.0-or-later

//! Scoped test counters and pipeline; worker batches retain the injected counters.

use super::{Counters, Message, Pipe};
use std::cell::Cell;
use std::{
    ops::Range,
    sync::{atomic::Ordering, mpsc, OnceLock},
};

thread_local! {
    static CURRENT: Cell<Option<&'static State>> = const { Cell::new(None) };
    static WORKER_COUNTERS: Cell<Option<&'static Counters>> = const { Cell::new(None) };
}

pub(super) struct State {
    pub(super) counters: &'static Counters,
    pub(super) pipe: OnceLock<Pipe>,
}

pub(super) fn state() -> Option<&'static State> {
    CURRENT.with(Cell::get)
}

pub(super) fn counters() -> Option<&'static Counters> {
    WORKER_COUNTERS.get().or_else(|| state().map(|state| state.counters))
}

pub(super) struct WorkerCounters(Option<&'static Counters>, std::marker::PhantomData<std::rc::Rc<()>>);

impl WorkerCounters {
    pub(super) fn enter(counters: &'static Counters) -> Self {
        Self(WORKER_COUNTERS.replace(Some(counters)), Default::default())
    }
}

impl Drop for WorkerCounters {
    fn drop(&mut self) {
        WORKER_COUNTERS.set(self.0);
    }
}

pub(crate) struct Scope {
    previous: Option<&'static State>,
    state: &'static State,
    // A scope must be dropped on the thread where it was installed.
    thread: std::marker::PhantomData<std::rc::Rc<()>>,
}

impl Scope {
    pub(crate) fn new() -> Self {
        Self::with_counters(Counters::default())
    }

    pub(crate) fn with_counters(counters: Counters) -> Self {
        // Pipe's existing test API requires static counters. One bounded instance per scope;
        // workers are joined at scope exit, including unwinding.
        let state = Box::leak(Box::new(State { counters: Box::leak(Box::new(counters)), pipe: OnceLock::new() }));
        Self { previous: CURRENT.with(|c| c.replace(Some(state))), state, thread: Default::default() }
    }
    /// Install the real intake with a receiver the test can drive on its own thread.
    pub(crate) fn install_intake(&self) -> Intake {
        self.intake(0)
    }

    /// Observe the intake before releasing each accepted batch to a real deposit worker.
    pub(crate) fn gated_worker(&self) -> Intake {
        self.intake(1)
    }

    fn intake(&self, count: usize) -> Intake {
        assert!(self.state.pipe.get().is_none());
        let pipe = self.state.pipe.get_or_init(|| Pipe::start(count, 0, self.state.counters));
        let (sender, receiver) = mpsc::sync_channel(1);
        let mut workers = oos_hal::sync::lock(&pipe.workers);
        let worker = workers.sender.replace(sender);
        assert_eq!(worker.is_some(), count != 0);
        Intake { pipe, receiver, worker }
    }

    /// Select one of two live fixtures for paired observations; scopes still drop in reverse creation order.
    pub(crate) fn select(&self) {
        CURRENT.with(|c| c.set(Some(self.state)));
    }
}

impl Drop for Scope {
    fn drop(&mut self) {
        if let Some(pipe) = self.state.pipe.get() {
            super::shutdown::close(pipe, std::time::Instant::now() + std::time::Duration::from_secs(5));
        }
        CURRENT.with(|c| c.set(self.previous));
    }
}

pub(crate) struct Intake {
    pipe: &'static Pipe,
    receiver: mpsc::Receiver<Message>,
    worker: Option<mpsc::SyncSender<Message>>,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Submission {
    range: Range<usize>,
    first_words: Vec<u16>,
    newest_seq: Option<u64>,
}

impl Submission {
    pub(crate) fn range(&self) -> Range<usize> {
        self.range.clone()
    }

    pub(crate) fn first_words(&self) -> &[u16] {
        &self.first_words
    }

    pub(crate) fn newest_seq(&self) -> Option<u64> {
        self.newest_seq
    }
}

impl Intake {
    /// Drain a batch, forwarding it to the worker if one was installed.
    pub(crate) fn take(&self) -> Option<Submission> {
        let mut batch = loop {
            match self.receiver.try_recv().ok()? {
                Message::Batch(batch) => break batch,
                Message::Wake => {
                    if let Some(worker) = &self.worker {
                        worker.send(Message::Wake).unwrap();
                    } else {
                        oos_hal::sync::lock(&self.pipe.workers).wake_pending.store(false, Ordering::Release);
                    }
                }
                Message::Barrier(_) => panic!("unexpected worker barrier"),
            }
        };
        let range = batch.next.load(Ordering::Relaxed)..batch.frames;
        let bytes = batch.lease.raw.as_ref().unwrap().bytes();
        let first_words =
            range.clone().map(|index| u16::from_le_bytes(bytes[index * batch.fb..][..2].try_into().unwrap())).collect();
        let submission = Submission { range, first_words, newest_seq: batch.newest.as_ref().map(|frame| frame.seq) };
        if let Some(worker) = &self.worker {
            let batch_mut = std::sync::Arc::get_mut(&mut batch).unwrap();
            batch_mut.clock = batch_mut.clock.pool_for_test(std::time::Duration::from_millis(1));
            worker.send(Message::Batch(batch)).unwrap();
            super::host::worker_barrier(worker);
        } else {
            batch.next.store(batch.frames, Ordering::Relaxed);
            oos_hal::sync::lock(&self.pipe.workers).queued.fetch_sub(1, Ordering::AcqRel);
        }
        Some(submission)
    }
}

#[test]
fn scopes_restore_counters_and_exclude_other_threads() {
    let outer = super::counters();
    let scope = Scope::new();
    Counters::add(&super::counters().acquired, 3);
    std::thread::spawn(|| {
        let _other = Scope::new();
        Counters::add(&super::counters().acquired, 7);
        assert_eq!(super::counters().read(), (7, 0, 0));
    })
    .join()
    .unwrap();
    assert_eq!(super::counters().read(), (3, 0, 0));
    {
        let _nested = Scope::new();
        assert_eq!(super::counters().read(), (0, 0, 0));
    }
    assert_eq!(super::counters().read(), (3, 0, 0));
    drop(scope);
    assert!(std::ptr::eq(super::counters(), outer));
}
