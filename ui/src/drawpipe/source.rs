// SPDX-License-Identifier: GPL-2.0-or-later

//! The DMA return and acquisition commands share one source event.

use super::pipe;
#[cfg(test)]
use super::Pipe;
use oos_hal::sync::lock;
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

#[derive(Default)]
pub(crate) struct SourceWake {
    generation: Mutex<u64>,
    changed: Condvar,
}

impl SourceWake {
    pub(crate) fn notify(&self) {
        let mut generation = lock(&self.generation);
        *generation = generation.wrapping_add(1);
        self.changed.notify_one();
    }

    pub(crate) fn generation(&self) -> u64 {
        *lock(&self.generation)
    }

    pub(crate) fn wait(&self, observed: u64, deadline: Duration) -> Option<Duration> {
        let generation = lock(&self.generation);
        if *generation != observed {
            return None;
        }
        let started = Instant::now();
        drop(self.changed.wait_timeout_while(generation, deadline, |seen| *seen == observed));
        Some(started.elapsed())
    }
}

pub(crate) fn source_event() -> Option<&'static SourceWake> {
    pipe().map(|pipe| pipe.source.as_ref())
}

pub(crate) fn wake_source() {
    if let Some(pipe) = pipe() {
        pipe.source.notify();
        pipe.slot.wake_tap_commands();
    }
}

#[cfg(test)]
impl Pipe {
    pub(crate) fn wait_tap_sender(&self) {
        self.slot.wait_for_sender();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drawpipe::{self, tests::Bytes, Counters, Lease};
    use std::sync::Arc;

    #[test]
    fn only_the_last_holder_returns_dma_and_notifies_the_waiter() {
        let pipe = Pipe::start(0, 0, Box::leak(Box::new(Counters::default())));
        let lease = Arc::new(Lease {
            raw: Some(Box::new(Bytes(vec![1, 2, 3]))),
            home: pipe.home.clone(),
            source: pipe.source.clone(),
        });
        let worker = lease.clone();
        let observed = pipe.source_generation();
        drop(lease);
        assert_eq!(pipe.source_generation(), observed);
        assert!(pipe.take_buffer().is_none());
        std::thread::scope(|threads| {
            threads.spawn(move || drop(worker));
            let _ = pipe.wait_source(observed, crate::ui::lifecycle::test_wait::HANG_GUARD);
        });
        assert_ne!(pipe.source_generation(), observed);
        let done = pipe.take_buffer().unwrap().downcast::<Bytes>().ok().unwrap();
        assert_eq!(done.0, [1, 2, 3]);
    }

    #[test]
    fn published_command_uses_the_existing_pipe_event_without_a_buffer() {
        let _scope = drawpipe::test_scope::Scope::new();
        let pipe = crate::drawpipe::start_pipe(&crate::sysconf::Switches::default());
        let observed = pipe.source_generation();
        wake_source();
        assert!(pipe.wait_source(observed, Duration::ZERO).is_none());
        assert!(pipe.take_buffer().is_none());
    }

    #[test]
    fn a_full_intake_preserves_the_retry_of_a_pending_deposit() {
        use super::super::{
            tests::{batch, dec, model},
            Message,
        };
        let pipe = Pipe::start(1, 0, Box::leak(Box::default()));
        pipe.set_view(Some(&model(1e-6, false, false)), 97, 48, crate::sparse::Interp::Exact);
        let (bytes, fb, settings, params) = batch(2, 400, false);
        for count in [1, 2] {
            pipe.feed(Box::new(Bytes(bytes.clone())), fb, count, 0, dec(&settings, &params));
            drawpipe::host::barrier(&pipe);
        }
        let offline = Pipe::start(0, 0, pipe.counters);
        let stale = offline.feed_batch(
            Box::new(Bytes(bytes)),
            fb,
            0..1,
            (0, Some(Default::default())),
            dec(&settings, &params),
            None,
        );
        let (reply, received) = std::sync::mpsc::channel();
        let (tx, queued) = {
            let workers = lock(&pipe.workers);
            (workers.sender.clone().unwrap(), workers.queued.clone())
        };
        let view = lock(&pipe.view);
        tx.send(Message::Barrier(reply)).unwrap();
        // The barrier is dequeued but blocked on the view. The stale batch fills the channel.
        queued.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        tx.send(Message::Batch(stale)).unwrap();
        assert_eq!(pipe.slot.take().unwrap().records, 1);
        pipe.wake();
        drop(view);
        received.recv().unwrap();
        drawpipe::host::barrier(&pipe);
        assert_eq!(pipe.slot.take().unwrap().records, 2);
    }
}

#[cfg(test)]
#[test]
fn repeated_ui_wakes_leave_room_for_all_raw_destinations() {
    let pipe = Pipe::start(0, 0, Box::leak(Box::default()));
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    lock(&pipe.workers).sender = Some(tx);
    for _ in 0..64 {
        pipe.wake();
    }
    assert!(matches!(rx.try_recv().unwrap(), super::Message::Wake));
    assert!(rx.try_recv().is_err(), "coalesced wakes cannot back up the DMA intake queue");
    lock(&pipe.workers).wake_pending.store(false, std::sync::atomic::Ordering::Release);
    pipe.wake();
    assert!(rx.try_recv().is_ok());
}
