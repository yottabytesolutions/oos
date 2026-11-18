// SPDX-License-Identifier: GPL-2.0-or-later
//! Bounded HTTP application intake and socket-free response ownership.
mod assets;
pub(crate) use assets::PAGE;
pub use assets::STYLE;
mod head;
pub(crate) use head::*;
pub use head::{parse_head, refuse_early, Head};
mod upload;
use super::{Envelope, Id, Ports, RemoteRequest, Reservation};
use core::net::SocketAddr;
use std::{
    sync::{
        atomic::{AtomicU64, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    task::{Poll, Waker},
};
pub(crate) use upload::Upload;

pub(crate) const SEND_INLINE: usize = 48 << 10;
const MAX_SENDERS: usize = 2;
/// Eight readers, eight screenshot waiters and two large responses.
pub(crate) const MAX_SESSIONS: usize = MAX_PENDING * 2 + MAX_SENDERS;
pub(crate) static SENDERS: AtomicUsize = AtomicUsize::new(0);

pub(crate) enum Body {
    Bytes(Vec<u8>),
    Upload(Option<Upload>),
}
impl Body {
    pub(crate) fn bytes(&self) -> &[u8] {
        match self {
            Self::Bytes(bytes) => bytes,
            Self::Upload(_) => &[],
        }
    }
    pub(crate) fn capacity(&self) -> usize {
        match self {
            Self::Bytes(bytes) => bytes.capacity(),
            Self::Upload(upload) => upload.as_ref().map_or(0, |job| job.len() + job.header().len()),
        }
    }
    pub(crate) fn take_upload(&mut self) -> Option<Upload> {
        match self {
            Self::Upload(upload) => upload.take(),
            Self::Bytes(_) => None,
        }
    }
}

pub struct Req {
    pub(crate) reply: ReplyHandle,
    pub(crate) peer: SocketAddr,
    pub(crate) head: hyper::Request<()>,
    pub(crate) body: Body,
}

pub(crate) struct Inbox {
    pub(crate) ports: Arc<Ports>,
    pub(crate) next: AtomicU64,
    pub(crate) active: AtomicUsize,
    pub(crate) large: super::UploadAdmission,
}
impl Default for Inbox {
    fn default() -> Self {
        let wake = Arc::new(super::Wake::new().expect("HTTP requires an intake wake descriptor"));
        Self {
            ports: Ports::with_limits(
                wake,
                MAX_PENDING,
                MAX_WAVEFORM + MAX_PENDING * (MAX_OTHER + 65536),
                MAX_SENDERS * MAX_WAVEFORM + MAX_PENDING * SEND_INLINE,
            )
            .expect("HTTP requires an IO wake descriptor"),
            next: AtomicU64::new(0),
            active: AtomicUsize::new(0),
            large: Default::default(),
        }
    }
}
impl Inbox {
    pub(crate) fn take(&self) -> Vec<Req> {
        self.ports.requests().wake().drain();
        self.ports
            .requests()
            .take(MAX_PENDING)
            .into_iter()
            .filter_map(|request| {
                if request.cancelled() {
                    return None;
                }
                let RemoteRequest::Http(req) = request.into_request() else { return None };
                req.reply.mark_taken();
                Some(req)
            })
            .collect()
    }
    pub(crate) fn envelope(&self, head: hyper::Request<()>, body: Body, peer: SocketAddr) -> Envelope {
        let id = Id::new(self.next.fetch_add(1, Ordering::Relaxed), 0);
        let reply = ReplyHandle(Arc::new(Mutex::new(State {
            ports: self.ports.clone(),
            taken: false,
            closed: false,
            reply: None,
            waker: None,
        })));
        Envelope::new(id, RemoteRequest::Http(Req { reply, peer, head, body }))
    }
}

#[derive(Clone)]
pub(crate) struct ReplyHandle(Arc<Mutex<State>>);
struct State {
    ports: Arc<Ports>,
    taken: bool,
    closed: bool,
    reply: Option<Response>,
    waker: Option<Waker>,
}
pub(crate) struct Response {
    pub(crate) head: hyper::http::response::Builder,
    body: super::output::Chunk,
    _reservation: Reservation,
    _sender: Option<SenderSlot>,
}
struct SenderSlot;
impl Drop for SenderSlot {
    fn drop(&mut self) {
        SENDERS.fetch_sub(1, Ordering::SeqCst);
    }
}
impl ReplyHandle {
    pub(crate) fn closed(&self) -> bool {
        oos_hal::sync::lock(&self.0).closed
    }
    pub(crate) fn close(&self) {
        let mut state = oos_hal::sync::lock(&self.0);
        state.closed = true;
        state.reply = None;
        if let Some(waker) = state.waker.take() {
            waker.wake();
        }
    }
    fn mark_taken(&self) {
        let mut state = oos_hal::sync::lock(&self.0);
        state.taken = true;
        if let Some(waker) = state.waker.take() {
            waker.wake();
        }
    }
    pub(crate) async fn taken(&self) {
        std::future::poll_fn(|cx| {
            let mut state = oos_hal::sync::lock(&self.0);
            if state.taken || state.closed {
                Poll::Ready(())
            } else {
                state.waker = Some(cx.waker().clone());
                Poll::Pending
            }
        })
        .await
    }
    pub(crate) async fn receive(&self) -> Option<Response> {
        std::future::poll_fn(|cx| {
            let mut state = oos_hal::sync::lock(&self.0);
            if let Some(reply) = state.reply.take() {
                Poll::Ready(Some(reply))
            } else if state.closed {
                Poll::Ready(None)
            } else {
                state.waker = Some(cx.waker().clone());
                Poll::Pending
            }
        })
        .await
    }
    pub(crate) fn send(
        &self,
        head: hyper::http::response::Builder,
        body: impl Into<super::output::Chunk>,
        force_large: bool,
    ) -> bool {
        let mut state = oos_hal::sync::lock(&self.0);
        if state.closed || state.reply.is_some() {
            return true;
        }
        let body = body.into();
        let sender = if force_large || body.bytes().len() > SEND_INLINE {
            if SENDERS.fetch_add(1, Ordering::SeqCst) >= MAX_SENDERS {
                SENDERS.fetch_sub(1, Ordering::SeqCst);
                return false;
            }
            Some(SenderSlot)
        } else {
            None
        };
        let Some(reservation) = state.ports.budget().reserve(body.capacity() + MAX_HEAD) else { return false };
        state.reply = Some(Response { head, body, _reservation: reservation, _sender: sender });
        if let Some(waker) = state.waker.take() {
            waker.wake();
        }
        true
    }
    pub(crate) fn content(&self, status: &str, kind: &str, body: Vec<u8>) -> bool {
        self.send(response(status).header("content-type", kind).header("cache-control", "no-store"), body, false)
    }
    pub(crate) fn static_content(&self, request: &hyper::Request<()>) -> bool {
        let head = parse_head(request);
        let (kind, body) = match head.route.as_str() {
            "/" => ("text/html; charset=utf-8", PAGE),
            "/style.css" => ("text/css", STYLE),
            _ => return false,
        };
        if !self.content("200 OK", kind, body.as_bytes().to_vec()) {
            self.answer("503 Service Unavailable", "busy, try again");
        }
        true
    }
    pub(crate) fn answer(&self, status: &str, message: &str) {
        if !self.send(response(status).header("content-type", "text/plain"), message.as_bytes().to_vec(), false) {
            self.close();
        }
    }
}

impl AsRef<[u8]> for Response {
    fn as_ref(&self) -> &[u8] {
        self.body.bytes()
    }
}
pub(crate) fn response(status: &str) -> hyper::http::response::Builder {
    hyper::Response::builder().status(status.split_whitespace().next().unwrap_or("500")).header("connection", "close")
}

#[cfg(test)]
pub(crate) fn request(text: &str) -> hyper::Request<()> {
    let mut lines = text.lines();
    let mut words = lines.next().unwrap_or("GET / HTTP/1.1").split_whitespace();
    let mut builder = hyper::Request::builder().method(words.next().unwrap_or("GET")).uri(words.next().unwrap_or("/"));
    for line in lines.filter(|line| !line.is_empty()) {
        if let Some((name, value)) = line.split_once(':') {
            builder = builder.header(name.trim(), value.trim());
        }
    }
    builder.body(()).unwrap()
}

#[cfg(test)]
pub(crate) fn senders_alone() -> std::sync::MutexGuard<'static, ()> {
    static ALONE: Mutex<()> = Mutex::new(());
    oos_hal::sync::lock(&ALONE)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exhausted_error_response_budget_closes_and_wakes_the_reply() {
        use std::{future::Future, task::Wake};
        #[derive(Default)]
        struct Woken(std::sync::atomic::AtomicBool);
        impl Wake for Woken {
            fn wake(self: Arc<Self>) {
                self.0.store(true, Ordering::SeqCst);
            }
        }
        let inbox = Inbox::default();
        let reservation = inbox.ports.budget().reserve_available().unwrap();
        let request = inbox.envelope(Default::default(), Body::Bytes(Vec::new()), "192.0.2.1:1".parse().unwrap());
        let RemoteRequest::Http(req) = request.into_request() else { panic!("HTTP request expected") };
        let reply = req.reply;
        assert!(!reply.content("200 OK", "text/plain", b"state".to_vec()));
        let woken = Arc::new(Woken::default());
        let waker = Waker::from(woken.clone());
        let mut cx = std::task::Context::from_waker(&waker);
        let mut response = std::pin::pin!(reply.receive());
        assert!(response.as_mut().poll(&mut cx).is_pending());
        reply.answer("503 Service Unavailable", "busy, try again");
        assert!(reply.closed());
        assert!(woken.0.load(Ordering::SeqCst));
        assert!(matches!(response.as_mut().poll(&mut cx), Poll::Ready(None)));
        drop(reservation);
        assert_eq!(inbox.ports.budget().used(), 0);
    }

    #[test]
    fn full_acknowledgement_channel_never_blocks_frame_intake() {
        let inbox = Inbox::default();
        let request = inbox.envelope(Default::default(), Body::Bytes(Vec::new()), "192.0.2.1:1".parse().unwrap());
        let RemoteRequest::Http(req) = request.request() else { panic!("HTTP request expected") };
        let reply = req.reply.clone();
        reply.mark_taken();
        assert!(inbox.ports.requests().submit(request.id(), request, 0).is_ok());
        let (done, completed) = std::sync::mpsc::channel();
        let frame = std::thread::spawn(move || {
            assert_eq!(inbox.take().len(), 1);
            done.send(()).unwrap();
        });
        completed
            .recv_timeout(crate::ui::lifecycle::test_wait::HANG_GUARD)
            .expect("frame intake must not wait for acknowledgement space");
        frame.join().unwrap();
        use std::{
            future::Future,
            task::{Context, Poll, Waker},
        };
        assert!(matches!(std::pin::pin!(reply.taken()).poll(&mut Context::from_waker(Waker::noop())), Poll::Ready(())));
    }
}
