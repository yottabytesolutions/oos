// SPDX-License-Identifier: GPL-2.0-or-later

//! Scoped test counters and pipeline; worker batches retain the injected counters.

use super::{Counters, Pipe};
use std::cell::Cell;
use std::sync::OnceLock;

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
