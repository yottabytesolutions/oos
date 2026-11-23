// SPDX-License-Identifier: GPL-2.0-or-later

use oos_hal::sync::lock;
use std::sync::{Arc, Condvar, Mutex};

/// Each worker has one reply in flight, so its reply needs only one reusable slot.
#[repr(align(64))]
pub(super) struct Reply<T> {
    state: Mutex<State<T>>,
    ready: Condvar,
}

struct State<T> {
    value: Option<T>,
    waiting: bool,
    closed: bool,
}

impl<T> Reply<T> {
    pub(super) fn new() -> Self {
        let reply =
            Self { state: Mutex::new(State { value: None, waiting: false, closed: false }), ready: Condvar::new() };
        // The pthread implementations on macOS allocate on first use.
        drop(lock(&reply.state));
        reply.ready.notify_one();
        reply
    }

    pub(super) fn recv(&self) -> Option<T> {
        self.recv_with(|| {})
    }

    fn recv_with(&self, on_wait: impl FnOnce()) -> Option<T> {
        let mut state = lock(&self.state);
        if state.value.is_none() && !state.closed {
            state.waiting = true;
            on_wait();
            state = self.ready.wait_while(state, |s| s.value.is_none() && !s.closed).unwrap_or_else(|e| e.into_inner());
            state.waiting = false;
        }
        state.value.take()
    }
}

pub(super) struct ReplySender<T>(Arc<Reply<T>>);

impl<T> ReplySender<T> {
    pub(super) fn new(reply: Arc<Reply<T>>) -> Self {
        Self(reply)
    }

    /// The worker sends once per received job, and the coordinator takes the reply before dispatching the next job.
    pub(super) fn send(&self, value: T) {
        let waiting = {
            let mut state = lock(&self.0.state);
            debug_assert!(state.value.is_none(), "the coordinator takes the previous reply before sending work");
            state.value = Some(value);
            state.waiting
        };
        if waiting {
            self.0.ready.notify_one();
        }
    }
}

impl<T> Drop for ReplySender<T> {
    fn drop(&mut self) {
        lock(&self.0.state).closed = true;
        self.0.ready.notify_one();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system::test_allocator::count_own;
    use std::sync::Barrier;

    #[test]
    fn reply_slots_are_cache_line_aligned() {
        assert_eq!(std::mem::align_of::<Reply<()>>(), 64);
    }

    #[test]
    fn waiting_receiver_returns_none_when_sender_drops() {
        let gate = Barrier::new(2);
        let reply = Arc::new(Reply::<()>::new());
        let sender = ReplySender::new(reply.clone());
        std::thread::scope(|scope| {
            let worker = scope.spawn(|| {
                gate.wait();
                drop(sender);
            });
            let value = reply.recv_with(|| {
                gate.wait();
            });
            assert_eq!(value, None);
            worker.join().unwrap();
        });
    }

    #[test]
    fn waiting_receiver_returns_none_when_sender_unwinds() {
        let gate = Barrier::new(2);
        let reply = Arc::new(Reply::<()>::new());
        let sender = ReplySender::new(reply.clone());
        std::thread::scope(|scope| {
            let worker = scope.spawn(|| {
                let _sender = sender;
                gate.wait();
                panic!("worker panics while owning the reply sender");
            });
            let value = reply.recv_with(|| {
                gate.wait();
            });
            assert_eq!(value, None);
            assert!(worker.join().is_err());
        });
    }

    #[test]
    fn replies_allocate_nothing_waiting_or_ready() {
        const WORKERS: usize = 3;
        const ROUNDS: usize = 64;
        let gate = Barrier::new(2);
        let replies: [_; WORKERS] = std::array::from_fn(|_| Arc::new(Reply::new()));
        let senders = replies.each_ref().map(|slot| ReplySender::new(slot.clone()));
        let results = std::thread::scope(|scope| {
            let worker = scope.spawn(|| {
                let mut allocations = 0;
                gate.wait();
                for round in 0..ROUNDS {
                    for (index, sender) in senders.iter().enumerate() {
                        gate.wait();
                        allocations += count_own(|| sender.send((round, index))).0;
                    }
                    gate.wait();
                    for (index, sender) in senders.iter().enumerate() {
                        allocations += count_own(|| sender.send((round, index))).0;
                    }
                    gate.wait();
                }
                allocations
            });
            // Initialize the test barrier outside the measurement.
            gate.wait();
            let (mut allocations, mut bad_replies, mut ready_waits) = (0, 0, 0);
            for round in 0..ROUNDS {
                for (index, reply) in replies.iter().enumerate() {
                    let (count, value) = count_own(|| {
                        reply.recv_with(|| {
                            gate.wait();
                        })
                    });
                    allocations += count;
                    bad_replies += usize::from(value != Some((round, index)));
                }
                gate.wait();
                gate.wait();
                for (index, reply) in replies.iter().enumerate() {
                    let (count, value) = count_own(|| reply.recv_with(|| ready_waits += 1));
                    allocations += count;
                    bad_replies += usize::from(value != Some((round, index)));
                }
            }
            (worker.join().unwrap(), allocations, bad_replies, ready_waits)
        });
        assert_eq!(results, (0, 0, 0, 0), "sender allocations, receiver allocations, wrong replies, ready waits");
    }
}
