// SPDX-License-Identifier: GPL-2.0-or-later

use std::sync::mpsc::{sync_channel, SyncSender};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

mod reply;
use reply::{Reply, ReplySender};

pub(crate) struct Timing {
    pub(crate) wall: Duration,
    pub(crate) worker_time: Duration,
}

fn run_shard<J, S>(jobs: &mut [J], scratch: &mut S, run: fn(&mut S, &mut J), timed: bool) -> Duration {
    let started = (timed && !jobs.is_empty()).then(Instant::now);
    for job in jobs {
        run(scratch, job);
    }
    started.map_or(Duration::ZERO, |at| at.elapsed())
}

/// Persistent synchronous pool. Arguments and results move by value, including pixel bands.
pub(crate) struct Pool<J, S> {
    workers: Vec<Worker<J>>,
    shards: Vec<Vec<J>>,
    run: fn(&mut S, &mut J),
    scratch: S,
}

struct Worker<J> {
    send: SyncSender<(Vec<J>, bool)>,
    reply: Arc<Reply<(Vec<J>, Duration)>>,
    join: JoinHandle<()>,
}

impl<J: Send + 'static, S: Default + Send + 'static> Pool<J, S> {
    pub(crate) fn new(workers: usize, little: u64, run: fn(&mut S, &mut J)) -> Self {
        let mut pool = Self { workers: Vec::new(), shards: Vec::new(), run, scratch: S::default() };
        for k in 1..workers {
            let (tx, rx) = sync_channel::<(Vec<J>, bool)>(1);
            let done = Arc::new(Reply::new());
            let reply = ReplySender::new(done.clone());
            match std::thread::Builder::new().name(format!("deposit{k}")).spawn(move || {
                crate::placement::to_little_cores(little);
                let mut scratch = S::default();
                while let Ok((mut jobs, timed)) = rx.recv() {
                    let elapsed = run_shard(&mut jobs, &mut scratch, run, timed);
                    reply.send((jobs, elapsed));
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

    pub(crate) fn map(&mut self, jobs: &mut Vec<J>, time_shards: bool) -> Timing {
        let started = Instant::now();
        let n = self.shards.len();
        for (i, job) in jobs.drain(..).enumerate() {
            self.shards[i % n].push(job);
        }
        for (worker, shard) in self.workers.iter().zip(&mut self.shards) {
            // A worker owns its receiver until pool shutdown, after every synchronous call returns.
            worker
                .send
                .send((std::mem::take(shard), time_shards))
                .expect("pool workers live until the coordinator closes intake");
        }
        let mut worker_time = run_shard(&mut self.shards[n - 1], &mut self.scratch, self.run, time_shards);
        jobs.append(&mut self.shards[n - 1]);
        for (worker, shard) in self.workers.iter().zip(&mut self.shards[..n - 1]) {
            let (returned, elapsed) =
                worker.reply.recv().expect("each pool worker returns its shard before the next phase");
            *shard = returned;
            worker_time += elapsed;
            jobs.append(shard);
        }
        Timing { wall: started.elapsed(), worker_time }
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
        pool.map(&mut jobs, false);
        pool.map(&mut jobs, false);
        jobs.sort_unstable();
        assert_eq!(jobs, (2..66).collect::<Vec<_>>());
    }

    #[test]
    #[should_panic(expected = "each pool worker returns its shard")]
    fn failed_worker_disconnects_its_own_reply() {
        let mut pool = Pool::new(2, 0, |_: &mut (), value: &mut u32| {
            assert_ne!(*value, 0);
        });
        pool.map(&mut vec![0, 1], false);
    }
}
