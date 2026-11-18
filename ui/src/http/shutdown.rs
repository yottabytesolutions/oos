// SPDX-License-Identifier: GPL-2.0-or-later
//! The screenshot encoder is the only HTTP CPU worker; sockets belong to netio.
use super::*;
use std::{io, thread::JoinHandle};
#[derive(Default)]
pub(super) struct Tasks(Mutex<State>);
#[derive(Default)]
struct State {
    closed: bool,
    workers: Vec<JoinHandle<()>>,
}
impl Tasks {
    pub(super) fn spawn(&self, name: &str, work: impl FnOnce() + Send + 'static) -> io::Result<()> {
        let mut state = oos_hal::sync::lock(&self.0);
        if state.closed {
            return Err(io::ErrorKind::Interrupted.into());
        }
        let mut i = 0;
        while i < state.workers.len() {
            if state.workers[i].is_finished() {
                let _ = state.workers.swap_remove(i).join();
            } else {
                i += 1;
            }
        }
        state.workers.push(std::thread::Builder::new().name(name.into()).spawn(work)?);
        Ok(())
    }
    pub(super) fn close(&self, deadline: Instant) -> usize {
        let workers = {
            let mut state = oos_hal::sync::lock(&self.0);
            state.closed = true;
            std::mem::take(&mut state.workers)
        };
        crate::system::shutdown::join(workers, deadline)
    }
}
impl Http {
    pub fn release_buffers(&mut self) {
        let mut state = self.shots.lock();
        state.png = None;
        state.next = None;
        state.px = Vec::new();
        state.spare = Vec::new();
        state.asked = None;
    }
    pub fn shutdown(&mut self, deadline: Instant) -> usize {
        self.set_open(None, false);
        self.inbox.ports.close();
        {
            let mut state = self.shots.lock();
            state.quit = true;
            state.waiting.clear();
        }
        self.shots.wake.notify_all();
        self.shots.tasks.close(deadline)
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shutdown_joins_workers_and_prevents_late_spawns() {
        let tasks = Tasks::default();
        tasks.spawn("finished", || {}).unwrap();
        assert_eq!(tasks.close(Instant::now() + crate::ui::lifecycle::test_wait::HANG_GUARD), 0);
        assert_eq!(tasks.spawn("late", || {}).unwrap_err().kind(), io::ErrorKind::Interrupted);
    }

    #[test]
    fn memory_reset_releases_unsent_screen_snapshots() {
        use super::super::*;
        let mut http = Http {
            open: None,
            runtime: None,
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
