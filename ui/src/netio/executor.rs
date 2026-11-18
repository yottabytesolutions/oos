// SPDX-License-Identifier: GPL-2.0-or-later
//! Shared lifetime of the single IO executor, independent of service switches.
use std::{io, sync::Arc, thread::JoinHandle};

#[derive(Clone)]
pub struct Runtime(Arc<Executor>);
struct Executor {
    handle: tokio::runtime::Handle,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<JoinHandle<()>>,
}
impl Runtime {
    pub fn new() -> io::Result<Self> {
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().max_blocking_threads(1).build()?;
        let handle = runtime.handle().clone();
        let (stop, stopped) = tokio::sync::oneshot::channel();
        let thread = std::thread::Builder::new().name("netio".into()).spawn(move || {
            runtime.block_on(async {
                let _ = stopped.await;
            });
        })?;
        Ok(Self(Arc::new(Executor { handle, stop: Some(stop), thread: Some(thread) })))
    }
    pub(super) fn spawn(&self, work: impl std::future::Future<Output = ()> + Send + 'static) {
        self.0.handle.spawn(work);
    }
}
impl Drop for Executor {
    fn drop(&mut self) {
        self.stop.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
