// SPDX-License-Identifier: GPL-2.0-or-later
//! A buffered result receiver shared by production polls and event-driven fixtures.
use std::cell::RefCell;
use std::sync::mpsc;

pub(crate) struct Receiver<T> {
    inner: mpsc::Receiver<T>,
    ready: RefCell<Option<T>>,
}
impl<T> From<mpsc::Receiver<T>> for Receiver<T> {
    fn from(inner: mpsc::Receiver<T>) -> Self {
        Self { inner, ready: RefCell::new(None) }
    }
}
impl<T> Receiver<T> {
    pub(crate) fn try_recv(&self) -> Result<T, mpsc::TryRecvError> {
        self.ready.borrow_mut().take().map_or_else(|| self.inner.try_recv(), Ok)
    }
    #[cfg(test)]
    pub(crate) fn wait_ready(&self, event: &str) {
        if self.ready.borrow().is_none() {
            let result = self
                .inner
                .recv_timeout(super::test_wait::HANG_GUARD)
                .unwrap_or_else(|error| panic!("waiting for {event}: {error}"));
            *self.ready.borrow_mut() = Some(result);
        }
    }
}

#[test]
fn awaiting_completion_preserves_the_result_for_the_real_poll() {
    let (send, receive) = mpsc::channel();
    let receive = Receiver::from(receive);
    let worker = std::thread::spawn(move || send.send(42).unwrap());
    receive.wait_ready("test worker result");
    receive.wait_ready("same buffered worker result");
    assert_eq!(receive.try_recv(), Ok(42));
    worker.join().unwrap();
    assert_eq!(receive.try_recv(), Err(mpsc::TryRecvError::Disconnected));
}

pub(in crate::ui) fn channel<T>() -> (mpsc::Sender<T>, Receiver<T>) {
    let (send, receive) = mpsc::channel();
    (send, receive.into())
}
pub(in crate::ui) fn sync_channel<T>(bound: usize) -> (mpsc::SyncSender<T>, Receiver<T>) {
    let (send, receive) = mpsc::sync_channel(bound);
    (send, receive.into())
}

#[test]
fn bounded_results_keep_backpressure_until_received() {
    let (send, receive) = sync_channel(1);
    send.try_send(1).unwrap();
    assert_eq!(send.try_send(2), Err(mpsc::TrySendError::Full(2)));
    assert_eq!(receive.try_recv(), Ok(1));
    send.try_send(2).unwrap();
    assert_eq!(receive.try_recv(), Ok(2));
}
