// SPDX-License-Identifier: GPL-2.0-or-later
//! Closing intake leaves all in-flight buffers owned until the coordinator and its pool finish.

use super::*;
use std::thread::JoinHandle;
use std::time::Instant;

#[derive(Default)]
pub(super) struct Workers {
    pub(super) sender: Option<SyncSender<Message>>,
    pub(super) joins: Vec<JoinHandle<()>>,
    pub(super) closed: bool,
}

impl Pipe {
    pub fn start(n: usize, little: u64, counters: &'static Counters) -> Pipe {
        let view = Arc::new(Mutex::new(ViewState::default()));
        let slot = Arc::new(Handoff::default());
        let mut workers = Workers::default();
        if n > 0 {
            let (tx, rx) = sync_channel(16);
            let (view, slot) = (view.clone(), slot.clone());
            match std::thread::Builder::new().name("deposit0".into()).spawn(move || {
                crate::placement::to_little_cores(little);
                stage::work(rx, &view, &slot, n, little, counters);
            }) {
                Ok(join) => {
                    workers.sender = Some(tx);
                    workers.joins.push(join);
                }
                Err(e) => eprintln!("oos-ui: deposit coordinator: {e}"),
            }
        }
        let (home, pool) = channel();
        Pipe { view, counters, workers: Mutex::new(workers), slot, live: Arc::default(), home, pool: Mutex::new(pool) }
    }
}

pub fn close(pipe: &Pipe, deadline: Instant) -> usize {
    pipe.slot.close();
    let joins = {
        let mut workers = oos_hal::sync::lock(&pipe.workers);
        workers.closed = true;
        workers.sender = None;
        std::mem::take(&mut workers.joins)
    };
    crate::system::shutdown::join(joins, deadline)
}

pub fn reset(pipe: &Pipe) {
    pipe.set_view(None, 0, 0, Interp::Exact);
    if let Some(deposit) = pipe.slot.take() {
        deposit.reject(oos_hal::sync::lock(&pipe.view).identity, pipe.counters);
    }
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
