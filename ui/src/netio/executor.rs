// SPDX-License-Identifier: GPL-2.0-or-later
//! Shared executor and service lifetimes. Every service inherits the executor's cancellation tree.
use std::{
    future::Future,
    io,
    sync::{mpsc, Arc},
    thread::JoinHandle,
};
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct Runtime(Arc<Executor>);
struct Executor {
    handle: tokio::runtime::Handle,
    stop: CancellationToken,
    thread: Option<JoinHandle<()>>,
}
impl Runtime {
    pub fn new() -> io::Result<Self> {
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().max_blocking_threads(1).build()?;
        let handle = runtime.handle().clone();
        let stop = CancellationToken::new();
        let stopped = stop.clone();
        let thread = std::thread::Builder::new().name("netio".into()).spawn(move || {
            runtime.block_on(stopped.cancelled());
        })?;
        Ok(Self(Arc::new(Executor { handle, stop, thread: Some(thread) })))
    }
    #[cfg(test)]
    pub(super) fn spawn(&self, work: impl Future<Output = ()> + Send + 'static) {
        self.0.handle.spawn(work);
    }
    pub(super) fn start<T: Send + 'static, F: Future<Output = ()> + Send + 'static>(
        &self,
        work: impl FnOnce(CancellationToken) -> io::Result<(T, F)> + Send + 'static,
    ) -> io::Result<Task<T>> {
        let (ready, started) = mpsc::sync_channel(1);
        let (finished, stopped) = mpsc::sync_channel(1);
        let stop = self.0.stop.child_token();
        let cancellation = stop.clone();
        self.0.handle.spawn(async move {
            match work(cancellation) {
                Ok((value, work)) => {
                    let _ = ready.send(Ok(value));
                    work.await;
                }
                Err(error) => {
                    let _ = ready.send(Err(error));
                }
            }
            let _ = finished.send(());
        });
        Ok(Task {
            local: started.recv().map_err(io::Error::other)??,
            stop,
            stopped: Some(stopped),
            _runtime: self.clone(),
        })
    }
}
impl Drop for Executor {
    fn drop(&mut self) {
        self.stop.cancel();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

pub(super) struct Task<T> {
    pub(super) local: T,
    stop: CancellationToken,
    stopped: Option<mpsc::Receiver<()>>,
    _runtime: Runtime,
}
impl<T> Task<T> {
    #[cfg(test)]
    pub(super) fn wait_stopped(&self) -> Result<(), mpsc::RecvTimeoutError> {
        self.stopped
            .as_ref()
            .ok_or(mpsc::RecvTimeoutError::Disconnected)?
            .recv_timeout(crate::ui::lifecycle::test_wait::HANG_GUARD)
    }
    pub(super) fn shutdown(&mut self, deadline: std::time::Instant) {
        self.stop.cancel();
        if let Some(stopped) = self.stopped.take() {
            let _ = stopped.recv_timeout(deadline.saturating_duration_since(std::time::Instant::now()));
        }
    }
}
impl<T> Drop for Task<T> {
    fn drop(&mut self) {
        self.stop.cancel();
        if let Some(stopped) = self.stopped.take() {
            let _ = stopped.recv();
        }
    }
}
