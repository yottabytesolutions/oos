// SPDX-License-Identifier: GPL-2.0-or-later
//! A named background thread that computes one job at a time off the frame loop.
//!
//! Admission counts outstanding jobs, including unconsumed results. Frame polls never block; shutdown drains do.
//! Dropping the worker closes its channels: the thread finishes the job in hand, then exits.
use super::worker_result;
use std::sync::mpsc::{self, TryRecvError, TrySendError};

pub(crate) struct Worker<J, R> {
    jobs: mpsc::SyncSender<J>,
    results: worker_result::Receiver<R>,
    pending: usize,
    capacity: usize,
}

impl<J: Send + 'static, R: Send + 'static> Worker<J, R> {
    pub(crate) fn spawn(name: &str, run: impl FnMut(J) -> R + Send + 'static) -> std::io::Result<Self> {
        Self::bounded(name, 1, run)
    }

    pub(crate) fn bounded(
        name: &str,
        capacity: usize,
        mut run: impl FnMut(J) -> R + Send + 'static,
    ) -> std::io::Result<Self> {
        if capacity == 0 {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "worker capacity must be positive"));
        }
        let (jobs, requests) = mpsc::sync_channel::<J>(capacity);
        let (finished, results) = worker_result::sync_channel(capacity);
        std::thread::Builder::new().name(name.into()).spawn(move || {
            while let Ok(job) = requests.recv() {
                if finished.send(run(job)).is_err() {
                    break;
                }
            }
        })?;
        Ok(Self { jobs, results, pending: 0, capacity })
    }
}

impl<J, R> Worker<J, R> {
    /// A job is out and its result not yet polled.
    pub(crate) fn busy(&self) -> bool {
        self.pending != 0
    }

    /// Hands `job` to the thread, unless capacity is exhausted (`Full`) or the thread is gone (`Disconnected`).
    pub(crate) fn submit(&mut self, job: J) -> Result<(), TrySendError<J>> {
        if self.pending == self.capacity {
            return Err(TrySendError::Full(job));
        }
        self.jobs.try_send(job)?;
        self.pending += 1;
        Ok(())
    }

    /// The result of the job that is out, once it is done. Never blocks.
    pub(crate) fn poll(&mut self) -> Result<R, TryRecvError> {
        let result = self.results.try_recv();
        if !matches!(result, Err(TryRecvError::Empty)) {
            self.pending = self.pending.saturating_sub(1);
        }
        result
    }

    /// Consume all outstanding results in order, blocking until each is available.
    pub(crate) fn drain(&mut self) -> impl Iterator<Item = Result<R, mpsc::RecvError>> + '_ {
        std::iter::from_fn(|| {
            if !self.busy() {
                return None;
            }
            let result = self.results.recv();
            self.pending -= 1;
            Some(result)
        })
    }

    /// Blocks until the job that is out is done; its result stays for the next `poll`.
    #[cfg(test)]
    pub(crate) fn wait_ready(&self, event: &str) {
        if self.busy() {
            self.results.wait_ready(event);
        }
    }

    /// A worker whose thread is the test: it takes the jobs and sends the results.
    #[cfg(test)]
    pub(crate) fn manual() -> (Self, mpsc::Receiver<J>, mpsc::SyncSender<R>) {
        let (jobs, requests) = mpsc::sync_channel(1);
        let (finished, results) = worker_result::sync_channel(1);
        (Self { jobs, results, pending: 0, capacity: 1 }, requests, finished)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_workers_count_unconsumed_results_and_drain_in_order() {
        let mut worker = Worker::bounded("worker-bounded", 3, |n: u8| n * 2).unwrap();
        for n in 1..=3 {
            worker.submit(n).unwrap();
        }
        worker.wait_ready("first bounded result");
        assert_eq!(worker.submit(4), Err(TrySendError::Full(4)));
        assert_eq!(worker.drain().next(), Some(Ok(2)));
        worker.submit(4).unwrap();
        assert_eq!(worker.drain().collect::<Result<Vec<_>, _>>().unwrap(), [4, 6, 8]);
        assert!(!worker.busy());
        assert!(worker.drain().next().is_none());
    }

    #[test]
    fn disconnected_results_release_credits_in_both_poll_and_drain() {
        let (mut worker, jobs, results) = Worker::<u8, u8>::manual();
        worker.submit(1).unwrap();
        assert_eq!(jobs.recv(), Ok(1));
        drop(results);
        assert_eq!(worker.poll(), Err(TryRecvError::Disconnected));
        assert!(!worker.busy());
        worker.submit(2).unwrap();
        assert!(worker.drain().next().unwrap().is_err());
        assert!(!worker.busy());
        drop(jobs);
        assert_eq!(worker.submit(3), Err(TrySendError::Disconnected(3)));
    }

    #[test]
    fn zero_outstanding_capacity_is_rejected() {
        assert_eq!(Worker::bounded("worker-zero", 0, |()| ()).err().unwrap().kind(), std::io::ErrorKind::InvalidInput);
    }

    #[test]
    fn one_job_is_out_until_its_result_is_polled() {
        let (release, gate) = mpsc::channel::<()>();
        let mut worker = Worker::spawn("worker-test", move |n: u32| {
            gate.recv().unwrap();
            n * 2
        })
        .unwrap();
        assert!(!worker.busy());
        worker.submit(1).unwrap();
        assert!(worker.busy());
        assert_eq!(worker.submit(2), Err(TrySendError::Full(2)));
        assert_eq!(worker.poll(), Err(TryRecvError::Empty));
        release.send(()).unwrap();
        worker.wait_ready("doubled job");
        assert!(worker.busy(), "waiting does not consume the result");
        assert_eq!(worker.poll(), Ok(2));
        assert!(!worker.busy());
        worker.submit(3).unwrap();
        release.send(()).unwrap();
        worker.wait_ready("second job");
        assert_eq!(worker.poll(), Ok(6));
    }

    #[test]
    fn the_thread_carries_its_name_and_state_between_jobs() {
        let mut count = 0;
        let mut worker = Worker::spawn("worker-name", move |()| {
            count += 1;
            (std::thread::current().name().map(String::from), count)
        })
        .unwrap();
        for expected in 1..=2 {
            worker.submit(()).unwrap();
            worker.wait_ready("named job");
            assert_eq!(worker.poll(), Ok((Some("worker-name".into()), expected)));
        }
    }

    #[test]
    fn dropping_the_worker_ends_the_thread_after_the_job_in_hand() {
        let (started, entered) = mpsc::channel::<()>();
        let (release, gate) = mpsc::channel::<()>();
        let (ended, exit) = mpsc::channel::<()>();
        struct Exit(mpsc::Sender<()>);
        impl Drop for Exit {
            fn drop(&mut self) {
                self.0.send(()).unwrap();
            }
        }
        let on_exit = Exit(ended);
        let mut worker = Worker::spawn("worker-drop", move |()| {
            let _ = &on_exit;
            started.send(()).unwrap();
            gate.recv().unwrap();
        })
        .unwrap();
        worker.submit(()).unwrap();
        entered.recv().unwrap();
        drop(worker);
        release.send(()).unwrap();
        exit.recv().unwrap();
    }

    #[test]
    fn a_manual_worker_hands_jobs_and_results_to_the_test() {
        let (mut worker, jobs, results) = Worker::<u8, u8>::manual();
        worker.submit(7).unwrap();
        assert_eq!(jobs.try_recv(), Ok(7));
        results.send(8).unwrap();
        assert_eq!(worker.poll(), Ok(8));
        drop(jobs);
        assert_eq!(worker.submit(9), Err(TrySendError::Disconnected(9)));
        assert!(!worker.busy());
    }
}
