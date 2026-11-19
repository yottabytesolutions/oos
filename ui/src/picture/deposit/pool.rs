// SPDX-License-Identifier: GPL-2.0-or-later

use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::thread::JoinHandle;

/// Persistent synchronous pool. Arguments and results move by value, including pixel bands.
pub(crate) struct Pool<J, S> {
    workers: Vec<Worker<J>>,
    shards: Vec<Vec<J>>,
    run: fn(&mut S, &mut J),
    scratch: S,
}

struct Worker<J> {
    send: SyncSender<Vec<J>>,
    reply: Receiver<Vec<J>>,
    join: JoinHandle<()>,
}

impl<J: Send + 'static, S: Default + Send + 'static> Pool<J, S> {
    pub(crate) fn new(workers: usize, little: u64, run: fn(&mut S, &mut J)) -> Self {
        let mut pool = Self { workers: Vec::new(), shards: Vec::new(), run, scratch: S::default() };
        for k in 1..workers {
            let (tx, rx) = sync_channel::<Vec<J>>(1);
            let (reply, done) = sync_channel(1);
            match std::thread::Builder::new().name(format!("deposit{k}")).spawn(move || {
                crate::placement::to_little_cores(little);
                let mut scratch = S::default();
                while let Ok(mut jobs) = rx.recv() {
                    for job in &mut jobs {
                        run(&mut scratch, job);
                    }
                    if reply.send(jobs).is_err() {
                        break;
                    }
                }
            }) {
                Ok(join) => {
                    pool.workers.push(Worker { send: tx, reply: done, join });
                    pool.shards.push(Vec::new());
                }
                Err(e) => eprintln!("oos-ui: deposit pool: {e}"),
            }
        }
        pool.shards.push(Vec::new());
        pool
    }

    pub(crate) fn map(&mut self, jobs: &mut Vec<J>) {
        let n = self.shards.len();
        for (i, job) in jobs.drain(..).enumerate() {
            self.shards[i % n].push(job);
        }
        for (worker, shard) in self.workers.iter().zip(&mut self.shards) {
            // A worker owns its receiver until pool shutdown, after every synchronous call returns.
            worker.send.send(std::mem::take(shard)).expect("pool workers live until the coordinator closes intake");
        }
        for job in &mut self.shards[n - 1] {
            (self.run)(&mut self.scratch, job);
        }
        jobs.append(&mut self.shards[n - 1]);
        for (worker, shard) in self.workers.iter().zip(&mut self.shards[..n - 1]) {
            *shard = worker.reply.recv().expect("each pool worker returns its shard before the next phase");
            jobs.append(shard);
        }
    }

    pub(crate) fn scratch(&mut self) -> &mut S {
        &mut self.scratch
    }
}

impl<J, S> Drop for Pool<J, S> {
    fn drop(&mut self) {
        for Worker { send, join, .. } in self.workers.drain(..) {
            drop(send);
            let _ = join.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_returns_every_owned_job() {
        let mut pool = Pool::new(4, 0, |_: &mut (), value: &mut u32| *value += 1);
        let mut jobs: Vec<_> = (0..64).collect();
        pool.map(&mut jobs);
        pool.map(&mut jobs);
        jobs.sort_unstable();
        assert_eq!(jobs, (2..66).collect::<Vec<_>>());
    }

    #[test]
    #[should_panic(expected = "each pool worker returns its shard")]
    fn failed_worker_disconnects_its_own_reply() {
        let mut pool = Pool::new(2, 0, |_: &mut (), value: &mut u32| {
            assert_ne!(*value, 0);
        });
        pool.map(&mut vec![0, 1]);
    }
}
