// SPDX-License-Identifier: GPL-2.0-or-later
//! HTTP admission and nonblocking frame-thread acknowledgements.
use super::*;

/// A complete request from a reader thread: its acknowledgement, the connection, the head (as sent) and the body.
pub(super) struct Req {
    pub(super) taken: std::sync::mpsc::SyncSender<()>,
    pub(super) s: TcpStream,
    pub(super) head: String,
    pub(super) body: Vec<u8>,
}

/// Requests read and waiting for the frame loop; the reader waits on `taken` until the frame loop took its request.
pub(super) struct Inbox {
    pub(super) port: crate::system::intake::Port<Req>,
    pub(super) next: std::sync::atomic::AtomicU64,
    /// Permanent process shutdown also wakes readers waiting for main intake.
    pub(super) quit: std::sync::atomic::AtomicBool,
    /// reader threads running
    pub(super) active: std::sync::atomic::AtomicUsize,
    /// a reader holds the one large body (`LARGE_BODY`)
    pub(super) large: std::sync::atomic::AtomicBool,
}

impl Default for Inbox {
    fn default() -> Self {
        Self {
            port: crate::system::intake::Port::new(
                MAX_PENDING,
                MAX_UPDATE.max(MAX_WAVEFORM) + MAX_PENDING * (MAX_OTHER + 65536),
                Arc::new(
                    crate::system::intake::Wake::new().expect("HTTP cannot serve requests without its intake wakeup"),
                ),
            ),
            next: Default::default(),
            quit: Default::default(),
            active: Default::default(),
            large: Default::default(),
        }
    }
}

impl Inbox {
    pub(super) fn take(&self) -> Vec<Req> {
        self.port.wake().drain();
        let requests = self.port.take(MAX_PENDING);
        for req in &requests {
            let _ = req.taken.try_send(());
        }
        requests
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system::intake::Id;
    use crate::ui::lifecycle::test_wait::HANG_GUARD;
    use std::sync::mpsc;

    #[test]
    fn full_acknowledgement_channel_never_blocks_frame_intake() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let _client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (socket, _) = listener.accept().unwrap();
        let inbox = Inbox::default();
        let (taken, reply) = mpsc::sync_channel(1);
        taken.try_send(()).unwrap();
        let request = Req { taken, s: socket, head: String::new(), body: Vec::new() };
        assert!(inbox.port.submit(Id::new(1, 0), request, 0).is_ok());
        let (done, completed) = mpsc::channel();
        let frame = std::thread::spawn(move || {
            assert_eq!(inbox.take().len(), 1);
            done.send(()).unwrap();
        });
        completed.recv_timeout(HANG_GUARD).expect("frame intake must not wait for reply space");
        frame.join().unwrap();
        assert_eq!(reply.try_recv(), Ok(()));
        assert_eq!(reply.try_recv(), Err(mpsc::TryRecvError::Disconnected));
    }
}
