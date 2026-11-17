// SPDX-License-Identifier: GPL-2.0-or-later
//! HTTP workers and their connections belong to one server, including senders.

use super::{lower_priority, MAX_SENDERS, SENDERS, SEND_TIMEOUT_S};
use std::io::{self, Write};
use std::net::{Shutdown, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;
use std::time::Instant;

#[derive(Default)]
pub(super) struct Tasks(Mutex<State>);

#[derive(Default)]
struct State {
    closed: bool,
    workers: Vec<Worker>,
}

struct Worker {
    join: JoinHandle<()>,
    socket: Arc<Mutex<Option<TcpStream>>>,
}

/// The connection clone must close when the worker ends, even if no later
/// request arrives to reap its JoinHandle. It also closes during unwinding.
struct Connection<T>(Arc<Mutex<Option<T>>>);
impl<T> Drop for Connection<T> {
    fn drop(&mut self) {
        oos_hal::sync::lock(&self.0).take();
    }
}

impl Tasks {
    pub(super) fn spawn(
        &self,
        name: &str,
        socket: Option<&TcpStream>,
        work: impl FnOnce() + Send + 'static,
    ) -> io::Result<()> {
        let mut state = oos_hal::sync::lock(&self.0);
        if state.closed {
            return Err(io::ErrorKind::Interrupted.into());
        }
        let mut i = 0;
        while i < state.workers.len() {
            if state.workers[i].join.is_finished() {
                let _ = state.workers.swap_remove(i).join.join();
            } else {
                i += 1;
            }
        }
        let socket = Arc::new(Mutex::new(socket.map(TcpStream::try_clone).transpose()?));
        let owned = socket.clone();
        let join = std::thread::Builder::new().name(name.into()).spawn(move || {
            let _connection = Connection(owned);
            work();
        })?;
        state.workers.push(Worker { join, socket });
        Ok(())
    }

    pub(super) fn close(&self, deadline: Instant) -> usize {
        let workers = {
            let mut state = oos_hal::sync::lock(&self.0);
            state.closed = true;
            for worker in &state.workers {
                if let Some(socket) = oos_hal::sync::lock(&worker.socket).as_ref() {
                    let _ = socket.shutdown(Shutdown::Both);
                }
            }
            std::mem::take(&mut state.workers)
        };
        crate::system::shutdown::join(workers.into_iter().map(|worker| worker.join).collect(), deadline)
    }
}

impl super::Http {
    pub fn release_buffers(&mut self) {
        let mut state = self.shots.lock();
        state.png = None;
        state.next = None;
        state.px = Vec::new();
        state.spare = Vec::new();
        state.asked = None;
    }

    pub fn shutdown(&mut self, deadline: Instant) -> usize {
        self.inbox.quit.store(true, std::sync::atomic::Ordering::Release);
        self.inbox.port.clear(true);
        self.set_open(None, false);
        {
            let mut state = self.shots.lock();
            state.quit = true;
            for (socket, ..) in state.waiting.drain(..) {
                let _ = socket.shutdown(Shutdown::Both);
            }
        }
        self.shots.wake.notify_all();
        self.shots.tasks.close(deadline)
    }
}

/// A large answer in the background. False when too many run already, or no thread could be started.
pub(super) fn send_in_background(
    s: &TcpStream,
    head: String,
    body: impl AsRef<[u8]> + Send + 'static,
    little: u64,
    tasks: &Tasks,
) -> bool {
    use std::sync::atomic::Ordering::SeqCst;
    let Ok(mut c) = s.try_clone() else { return false };
    if SENDERS.fetch_add(1, SeqCst) >= MAX_SENDERS {
        SENDERS.fetch_sub(1, SeqCst);
        return false;
    }
    let t = tasks.spawn("web-send", Some(s), move || {
        lower_priority(little);
        let _ = c.set_write_timeout(Some(Duration::from_secs(SEND_TIMEOUT_S)));
        let _ = c.write_all(head.as_bytes());
        let _ = c.write_all(body.as_ref());
        SENDERS.fetch_sub(1, SeqCst);
    });
    if t.is_err() {
        SENDERS.fetch_sub(1, SeqCst);
        return false;
    }
    true
}

pub(super) fn queue_request(s: TcpStream, inbox: &super::Inbox, text: String, body: Vec<u8>, lim: super::Limits) {
    use super::{answer, Req};
    let id = crate::system::intake::Id::new(inbox.next.fetch_add(1, std::sync::atomic::Ordering::SeqCst), 0);
    let Ok(sc) = s.try_clone() else { return };
    let (taken, reply) = std::sync::mpsc::sync_channel(1);
    let bytes = text.capacity() + body.capacity();
    if inbox.quit.load(std::sync::atomic::Ordering::Acquire) {
        return;
    }
    if inbox.port.submit(id, Req { taken, s: sc, head: text, body }, bytes).is_err() {
        return answer(&s, "503 Service Unavailable", "busy, try again");
    }
    if matches!(reply.recv_timeout(lim.take), Err(std::sync::mpsc::RecvTimeoutError::Timeout)) && inbox.port.cancel(id)
    {
        answer(&s, "503 Service Unavailable", "busy, try again");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shutdown_joins_workers_and_prevents_late_spawns() {
        let tasks = Tasks::default();
        tasks.spawn("finished", None, || {}).unwrap();
        assert_eq!(tasks.close(Instant::now() + crate::ui::lifecycle::test_wait::HANG_GUARD), 0);
        assert_eq!(tasks.spawn("late", None, || {}).unwrap_err().kind(), io::ErrorKind::Interrupted);
    }

    #[test]
    fn completed_worker_closes_its_connection_before_handle_is_reaped() {
        let resource = Arc::new(vec![42; 1024]);
        let weak = Arc::downgrade(&resource);
        let slot = Arc::new(Mutex::new(Some(resource)));
        let own = slot.clone();
        let worker = std::thread::spawn(move || {
            let _connection = Connection(own);
        });
        // Joining waits for the connection's Drop, independent of scheduling.
        worker.join().unwrap();
        assert!(weak.upgrade().is_none());
        assert!(oos_hal::sync::lock(&slot).is_none());
    }

    #[test]
    fn memory_reset_releases_unsent_screen_snapshots() {
        use super::super::*;
        let mut http = Http {
            open: None,
            inbox: Default::default(),
            limits: LIMITS,
            shots: Shots::new(0),
            acl: Default::default(),
            acl_shared: Default::default(),
            offer: Default::default(),
        };
        {
            let mut state = http.shots.lock();
            state.next = Some((32, 32, vec![42; 1024]));
            state.px = vec![42; 1024];
            state.spare = vec![0; 1024];
            state.png = Some(Arc::from([42u8; 1024]));
        }
        http.release_buffers();
        let state = http.shots.lock();
        assert!(state.next.is_none() && state.png.is_none());
        assert_eq!((state.px.capacity(), state.spare.capacity()), (0, 0));
    }
}
