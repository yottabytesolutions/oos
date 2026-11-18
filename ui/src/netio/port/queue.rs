// SPDX-License-Identifier: GPL-2.0-or-later
//! Synchronous owned request/reply queues. Rejected values remain owned by the sender.
pub use super::Wake;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Id {
    connection: u64,
    request: u64,
}

impl Id {
    pub fn new(connection: u64, request: u64) -> Self {
        Self { connection, request }
    }

    pub fn connection(self) -> u64 {
        self.connection
    }

    pub fn request(self) -> u64 {
        self.request
    }
}

pub struct Port<T> {
    state: Mutex<State<T>>,
    wake: Arc<Wake>,
    entries: usize,
    bytes: usize,
    ready: tokio::sync::Notify,
}

struct State<T> {
    queue: VecDeque<(Id, T, usize)>,
    bytes: usize,
    closed: bool,
}

impl<T> Port<T> {
    pub fn new(entries: usize, bytes: usize, wake: Arc<Wake>) -> Self {
        Self {
            state: Mutex::new(State { queue: VecDeque::new(), bytes: 0, closed: false }),
            wake,
            entries,
            bytes,
            ready: Default::default(),
        }
    }

    /// Charge retained allocation capacity, rather than just the payload length.
    pub fn submit(&self, id: Id, value: T, bytes: usize) -> Result<(), T> {
        let mut state = oos_hal::sync::lock(&self.state);
        if state.closed || state.queue.len() >= self.entries || bytes > self.bytes.saturating_sub(state.bytes) {
            return Err(value);
        }
        state.bytes += bytes;
        state.queue.push_back((id, value, bytes));
        self.wake.notify();
        self.ready.notify_one();
        Ok(())
    }

    /// Rearm work left by the quota. The frame owner drains the wake before checking its ports.
    /// Removing replies also wakes the frame owner to retry its parked, owned reply.
    pub fn take(&self, quota: usize) -> Vec<T> {
        let mut state = oos_hal::sync::lock(&self.state);
        let State { queue, bytes, .. } = &mut *state;
        let out: Vec<T> = queue
            .drain(..quota.min(queue.len()))
            .map(|(_, value, capacity)| {
                *bytes -= capacity;
                value
            })
            .collect();
        if !state.queue.is_empty() || !out.is_empty() {
            self.wake.notify();
        }
        out
    }

    /// True only if cancellation won the race with frame intake.
    pub fn cancel(&self, id: Id) -> bool {
        let mut state = oos_hal::sync::lock(&self.state);
        let bytes = state
            .queue
            .iter()
            .position(|(queued, _, _)| *queued == id)
            .and_then(|i| state.queue.remove(i))
            .map(|(_, _, bytes)| bytes);
        state.bytes -= bytes.unwrap_or(0);
        self.wake.notify();
        bytes.is_some()
    }

    pub fn clear(&self, close: bool) {
        let mut state = oos_hal::sync::lock(&self.state);
        state.closed |= close;
        state.queue.clear();
        state.bytes = 0;
        self.wake.notify();
    }

    pub fn wake_handle(&self) -> Arc<Wake> {
        self.wake.clone()
    }

    pub fn wake(&self) -> &Wake {
        &self.wake
    }

    pub(crate) async fn notified(&self) {
        self.ready.notified().await;
    }

    pub fn is_empty(&self) -> bool {
        oos_hal::sync::lock(&self.state).queue.is_empty()
    }
}

#[cfg(test)]
mod tests;
