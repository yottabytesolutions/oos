// SPDX-License-Identifier: GPL-2.0-or-later
//! Closing intake leaves all in-flight buffers owned until the coordinator and its pool finish.

use super::*;
use crate::picture::deposit::Handoff;
#[cfg(test)]
use crate::{model::Model, sparse::Interp};
use std::sync::atomic::AtomicUsize;
use std::sync::mpsc::{channel, sync_channel, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Instant;

#[derive(Default)]
pub(super) struct Workers {
    pub(super) sender: Option<SyncSender<Message>>,
    pub(super) joins: Vec<JoinHandle<()>>,
    pub(super) closed: bool,
    pub(super) queued: Arc<AtomicUsize>,
    pub(super) wake_pending: Arc<std::sync::atomic::AtomicBool>,
}

impl Pipe {
    pub(crate) fn batch_counters(&self) -> &'static Counters {
        self.counters
    }

    pub fn start(n: usize, little: u64, counters: &'static Counters) -> Pipe {
        let view = Arc::new(Mutex::new(ViewState::default()));
        let slot = Arc::new(Handoff::default());
        let mut workers = Workers::default();
        if n > 0 {
            let (tx, rx) = sync_channel(1);
            let (view, slot) = (view.clone(), slot.clone());
            let intake = (workers.queued.clone(), workers.wake_pending.clone());
            match std::thread::Builder::new().name("deposit0".into()).spawn(move || {
                crate::placement::to_little_cores(little);
                stage::work(rx, (&intake.0, &intake.1), &view, &slot, n, little, counters);
            }) {
                Ok(join) => {
                    workers.sender = Some(tx);
                    workers.joins.push(join);
                }
                Err(e) => eprintln!("oos-ui: deposit coordinator: {e}"),
            }
        }
        let (home, pool) = channel();
        Pipe {
            view,
            counters,
            workers: Mutex::new(workers),
            admission: Arc::default(),
            slot,
            live: Arc::default(),
            home,
            pool: Mutex::new(pool),
            source: Arc::default(),
        }
    }
}

pub fn close(pipe: &Pipe, deadline: Instant) -> usize {
    pipe.slot.close();
    pipe.source.notify();
    let joins = {
        let mut workers = oos_hal::sync::lock(&pipe.workers);
        workers.closed = true;
        workers.sender = None;
        std::mem::take(&mut workers.joins)
    };
    crate::system::shutdown::join(joins, deadline)
}

pub fn reset(pipe: &Pipe) {
    let (pending, identity) = {
        let mut state = oos_hal::sync::lock(&pipe.view);
        (state.view, state.key) = (None, None);
        // The first view published after reset has the next generation.
        (pipe.slot.reset(state.gen + 1), state.identity)
    };
    if let Some(deposit) = pending {
        deposit.reject(identity, pipe.counters);
    }
    pipe.wake();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closing_idle_pipe_joins_workers_and_refuses_intake() {
        let pipe = Pipe::start(2, 0, Box::leak(Box::default()));
        pipe.set_view(Some(&Model::default()), 16, 16, Interp::Exact);
        assert!(pipe.accepting());
        assert_eq!(close(&pipe, Instant::now() + crate::ui::lifecycle::test_wait::HANG_GUARD), 0);
        assert!(!pipe.accepting());
        assert_eq!(close(&pipe, Instant::now()), 0);
    }
}
