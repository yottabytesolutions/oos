// SPDX-License-Identifier: GPL-2.0-or-later
//! Closing deposit intake is serialized with publication to the workers.

use super::*;
use std::thread::JoinHandle;
use std::time::Instant;

#[derive(Default)]
pub(super) struct Workers {
    pub(super) senders: Vec<SyncSender<Arc<Batch>>>,
    pub(super) joins: Vec<JoinHandle<()>>,
    pub(super) closed: bool,
}

impl Pipe {
    /// A pipe with `n` workers counting into `counters`; the workers go to the cores of `little` (`Cores::little`).
    pub fn start(n: usize, little: u64, counters: &'static Counters) -> Pipe {
        let view: Arc<Mutex<ViewState>> = Default::default();
        let mut mails = Vec::new();
        let mut workers = shutdown::Workers::default();
        for k in 0..n {
            // batches are bounded by the read-out buffers (`hw::POOL`), a few at most
            let (tx, rx) = sync_channel::<Arc<Batch>>(16);
            let mail: Arc<Mutex<Mail>> = Default::default();
            let (v, m) = (view.clone(), mail.clone());
            match std::thread::Builder::new().name(format!("deposit{}", k)).spawn(move || {
                crate::http::to_little_cores(little);
                work(rx, &v, &m)
            }) {
                Ok(handle) => {
                    workers.senders.push(tx);
                    workers.joins.push(handle);
                    mails.push(mail);
                }
                Err(e) => eprintln!("oos-ui: deposit worker: {}", e),
            }
        }
        mails.push(Default::default());
        let (home, pool) = channel();
        Pipe {
            view,
            counters,
            workers: Mutex::new(workers),
            mails,
            open: Default::default(),
            live: Default::default(),
            ready: Default::default(),
            home,
            pool: Mutex::new(pool),
            merges: AtomicU64::new(0),
            seen: Default::default(),
            partial: AtomicU64::new(0),
        }
    }
}

pub fn close(pipe: &Pipe, deadline: Instant) -> usize {
    let joins = {
        let mut workers = oos_hal::sync::lock(&pipe.workers);
        workers.closed = true;
        workers.senders.clear();
        std::mem::take(&mut workers.joins)
    };
    crate::system::shutdown::join(joins, deadline)
}

pub fn reset(pipe: &Pipe) {
    pipe.set_view(None, 0, 0, Interp::Exact);
    for mail in &pipe.mails {
        let mut mail = oos_hal::sync::lock(mail);
        mail.full.clear();
        mail.spare.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn closing_idle_pipe_joins_workers_and_refuses_intake() {
        let pipe = Pipe::start(2, 0, Box::leak(Box::default()));
        pipe.set_view(Some(&Model::default()), 16, 16, Interp::Exact);
        assert!(pipe.accepting());
        assert_eq!(close(&pipe, Instant::now() + Duration::from_secs(1)), 0);
        assert!(!pipe.accepting());
        assert_eq!(close(&pipe, Instant::now()), 0);
    }
}
