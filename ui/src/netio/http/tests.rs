// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;
use std::{
    cell::Cell,
    future::Future,
    pin::Pin,
    task::{Context, Poll, Waker},
    time::Duration,
};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

#[derive(Default)]
struct Socket {
    input: Vec<u8>,
    output: Vec<u8>,
}
impl AsyncRead for Socket {
    fn poll_read(mut self: Pin<&mut Self>, _: &mut Context<'_>, buffer: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
        if self.input.is_empty() {
            return if self.output.is_empty() { Poll::Pending } else { Poll::Ready(Ok(())) };
        }
        let n = buffer.remaining().min(self.input.len());
        buffer.put_slice(&self.input[..n]);
        self.input.drain(..n);
        Poll::Ready(Ok(()))
    }
}
impl AsyncWrite for Socket {
    fn poll_write(mut self: Pin<&mut Self>, _: &mut Context<'_>, bytes: &[u8]) -> Poll<io::Result<usize>> {
        self.output.extend_from_slice(bytes);
        Poll::Ready(Ok(bytes.len()))
    }
    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
    fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}
fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap()
}

pub(crate) fn partial_head_deadline() {
    runtime().block_on(async {
        let mut socket = Socket { input: b"GET / HTTP/1.1\r\nHo".to_vec(), ..Default::default() };
        let samples = Cell::new(0u32);
        let inbox = Inbox::default();
        let acl = Default::default();
        let input = input::Input { inbox: &inbox, acl: &acl, peer: "127.0.0.1:1".parse().unwrap(), limits: LIMITS };
        let read = input.read(
            &mut socket,
            || {
                let n = samples.get();
                samples.set(n + 1);
                assert!(n <= 1, "deadline must stop the trickle before another read");
                LIMITS.deadline * n
            },
            |_| std::future::pending(),
        );
        assert!(read.await.unwrap().is_none());
        assert_eq!(samples.get(), 2);
        assert!(socket.output.starts_with(b"HTTP/1.1 408"));
        assert!(String::from_utf8_lossy(&socket.output).contains("request took too long"));
    });
}

#[test]
pub(crate) fn idle_partial_head_expires_only_on_injected_time() {
    runtime().block_on(async {
        let mut socket = Socket { input: b"GET / HTTP/1.1\r\nHo".to_vec(), ..Default::default() };
        let clock = &Cell::new(Duration::ZERO);
        let inbox = Inbox::default();
        let acl = Default::default();
        let input = input::Input { inbox: &inbox, acl: &acl, peer: "127.0.0.1:1".parse().unwrap(), limits: LIMITS };
        let mut read = Box::pin(input.read(
            &mut socket,
            || clock.get(),
            |deadline| {
                std::future::poll_fn(move |_| if clock.get() >= deadline { Poll::Ready(()) } else { Poll::Pending })
            },
        ));
        let mut cx = Context::from_waker(Waker::noop());
        assert!(read.as_mut().poll(&mut cx).is_pending());
        clock.set(LIMITS.idle);
        assert!(matches!(read.as_mut().poll(&mut cx), Poll::Ready(Ok(None))));
        drop(read);
        assert!(socket.output.starts_with(b"HTTP/1.1 408"));
    });
}

#[test]
fn queued_request_cancellation_returns_credit_and_discards_late_output() {
    let inbox = Inbox::default();
    let envelope = inbox.envelope(
        "GET /ui-state HTTP/1.1\r\n\r\n".into(),
        Body::Bytes(Vec::new()),
        "127.0.0.1:1".parse().unwrap(),
    );
    let id = envelope.id();
    let RemoteRequest::Http(req) = envelope.request() else { panic!("HTTP request expected") };
    let reply = req.reply.clone();
    let credit = envelope.credit(inbox.ports.clone());
    assert!(inbox.ports.requests().submit(id, envelope, 0).is_ok());
    drop(credit);
    assert!(inbox.take().is_empty());
    reply.close();
    assert!(reply.send("HTTP/1.1 200 OK\r\n\r\n".into(), vec![1; 4096], false));
    assert_eq!(inbox.ports.budget().used(), 0);
}

pub(crate) fn untaken_request_times_out() {
    runtime().block_on(async {
        let inbox = Inbox::default();
        let envelope = inbox.envelope(
            "GET /ui-state HTTP/1.1\r\n\r\n".into(),
            Body::Bytes(Vec::new()),
            "127.0.0.1:1".parse().unwrap(),
        );
        let id = envelope.id();
        let RemoteRequest::Http(req) = envelope.request() else { panic!("HTTP request expected") };
        let reply = req.reply.clone();
        let _credit = envelope.credit(inbox.ports.clone());
        assert!(inbox.ports.requests().submit(id, envelope, 0).is_ok());
        let clock = &Cell::new(Duration::ZERO);
        let timer = std::future::poll_fn(|_| if clock.get() >= LIMITS.take { Poll::Ready(()) } else { Poll::Pending });
        let mut taken = Box::pin(wait_taken(&reply, timer, || inbox.ports.requests().cancel(id)));
        let mut cx = Context::from_waker(Waker::noop());
        assert!(taken.as_mut().poll(&mut cx).is_pending());
        clock.set(LIMITS.take);
        assert!(matches!(taken.as_mut().poll(&mut cx), Poll::Ready(false)));
        assert!(inbox.ports.requests().is_empty());
        let mut socket = Socket::default();
        input::answer(&mut socket, "503 Service Unavailable", "busy, try again").await.unwrap();
        assert!(socket.output.starts_with(b"HTTP/1.1 503"));
    });
}

#[test]
fn completed_worker_closes_its_connection_before_handle_is_reaped() {
    let inbox = Inbox::default();
    let envelope = inbox.envelope(String::new(), Body::Bytes(Vec::new()), "127.0.0.1:1".parse().unwrap());
    let RemoteRequest::Http(req) = envelope.request() else { panic!("HTTP request expected") };
    let reply = req.reply.clone();
    let bytes: Arc<[u8]> = Arc::from(vec![42; 4096]);
    let weak = Arc::downgrade(&bytes);
    assert!(reply.send(String::new(), bytes, false));
    let worker = std::thread::spawn(move || {
        let _connection = Connection(reply);
    });
    worker.join().unwrap();
    assert!(weak.upgrade().is_none());
    assert_eq!(inbox.ports.budget().used(), 0);
}

#[test]
fn large_output_admission_and_cancellation_return_all_capacity() {
    let _alone = super::super::port::http::senders_alone();
    let inbox = Inbox::default();
    let replies: Vec<_> = (0..3)
        .map(|_| {
            let request = inbox.envelope(String::new(), Body::Bytes(Vec::new()), "127.0.0.1:1".parse().unwrap());
            let RemoteRequest::Http(req) = request.into_request() else { panic!("HTTP request expected") };
            req.reply
        })
        .collect();
    assert!(replies[0].send(String::new(), vec![1; SEND_INLINE + 1], false));
    assert!(replies[1].send(String::new(), vec![2; SEND_INLINE + 1], false));
    assert!(!replies[2].send(String::new(), vec![3; SEND_INLINE + 1], false));
    assert!(replies[2].send(String::new(), b"small".to_vec(), false));
    replies[0].close();
    assert_eq!(senders_in_use(), 1);
    replies[1].close();
    replies[2].close();
    assert_eq!(senders_in_use(), 0);
    assert_eq!(inbox.ports.budget().used(), 0);
}

#[test]
fn closing_service_joins_sessions_and_cancels_queued_intake() {
    let runtime = Runtime::new().unwrap();
    let inbox = Arc::new(Inbox::default());
    let service =
        Service::start(crate::net6::Port::Host, false, &runtime, inbox.clone(), Default::default(), LIMITS).unwrap();
    let mut client = std::net::TcpStream::connect(service.addr()).unwrap();
    use std::io::{Read, Write};
    client.write_all(b"GET /ui-state HTTP/1.1\r\n\r\n").unwrap();
    let wait = crate::ui::lifecycle::test_wait::Wait::new("HTTP queued request");
    while inbox.ports.requests().is_empty() {
        wait.poll();
    }
    drop(service);
    assert!(inbox.ports.requests().is_empty());
    assert_eq!(inbox.active.load(Ordering::SeqCst), 0);
    client.set_read_timeout(Some(crate::ui::lifecycle::test_wait::HANG_GUARD)).unwrap();
    assert_eq!(client.read(&mut [0]).unwrap(), 0);
}

#[test]
fn cancelling_a_large_body_read_releases_its_single_admission() {
    runtime().block_on(async {
        let inbox = Inbox::default();
        let acl = Default::default();
        let input = input::Input { inbox: &inbox, acl: &acl, peer: "127.0.0.1:1".parse().unwrap(), limits: LIMITS };
        let head = format!("POST /update HTTP/1.1\r\nContent-Length: {}\r\n\r\n", 2 * LARGE_BODY);
        let mut socket = Socket { input: head.as_bytes().to_vec(), ..Default::default() };
        let mut reading = Box::pin(input.read(&mut socket, || Duration::ZERO, |_| std::future::pending()));
        std::future::poll_fn(|cx| {
            assert!(reading.as_mut().poll(cx).is_pending());
            if inbox.large.occupied() {
                Poll::Ready(())
            } else {
                Poll::Pending
            }
        })
        .await;
        let mut other = Socket { input: head.into_bytes(), ..Default::default() };
        assert!(input.read(&mut other, || Duration::ZERO, |_| std::future::pending()).await.unwrap().is_none());
        assert!(other.output.starts_with(b"HTTP/1.1 503"));
        drop(reading);
        assert!(!inbox.large.occupied());
        assert!(inbox.ports.requests().is_empty());
    });
}

#[test]
fn half_closed_request_receives_its_application_response() {
    use std::io::{Read, Write};
    runtime().block_on(async {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let mut client = std::net::TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        client.set_read_timeout(Some(crate::ui::lifecycle::test_wait::HANG_GUARD)).unwrap();
        client.write_all(b"GET /ui-state HTTP/1.1\r\n\r\n").unwrap();
        client.shutdown(std::net::Shutdown::Write).unwrap();
        let (server, peer) = listener.accept().unwrap();
        server.set_nonblocking(true).unwrap();
        let mut socket = TcpStream::from_std(server).unwrap();
        let inbox = Arc::new(Inbox::default());
        inbox.active.store(1, Ordering::SeqCst);
        let mut session =
            Box::pin(conversation(&mut socket, peer, inbox.clone(), Default::default(), LIMITS, Active(inbox.clone())));
        std::future::poll_fn(|cx| {
            assert!(session.as_mut().poll(cx).is_pending(), "EOF must not cancel queued intake");
            if inbox.ports.requests().is_empty() {
                Poll::Pending
            } else {
                Poll::Ready(())
            }
        })
        .await;
        let mut requests = inbox.take();
        assert_eq!(requests.len(), 1);
        let reply = requests.pop().unwrap().reply;
        assert!(session.as_mut().poll(&mut Context::from_waker(Waker::noop())).is_pending());
        reply.answer("200 OK", "state");
        session.await.unwrap();
        drop(socket);
        let mut response = String::new();
        client.read_to_string(&mut response).unwrap();
        assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(response.ends_with("\r\n\r\nstate"));
        assert_eq!(inbox.active.load(Ordering::SeqCst), 0);
        assert_eq!(inbox.ports.budget().used(), 0);
    });
}

#[test]
fn signed_update_keeps_large_body_admission_and_verification_on_its_worker() {
    runtime().block_on(async {
        use oos_hal::update::{ed25519, pkg};
        let keys = [ed25519::public_key(&[9; 32])];
        let package = pkg::build(&[9; 32], "b", "k", &vec![42; 2 * LARGE_BODY]).unwrap();
        let length = package.len();
        let mut bytes = format!("POST /update HTTP/1.1\r\nContent-Length: {length}\r\n\r\n").into_bytes();
        bytes.extend_from_slice(&package);
        let mut socket = Socket { input: bytes, ..Default::default() };
        let inbox = Inbox::default();
        let acl = Default::default();
        let peer = SocketAddr::from((std::net::Ipv4Addr::LOCALHOST, 1));
        let input = input::Input { inbox: &inbox, acl: &acl, peer, limits: LIMITS };
        let (head, body, slot) =
            input.read(&mut socket, || Duration::ZERO, |_| std::future::pending()).await.unwrap().unwrap();
        assert!(slot.is_some());
        assert!(inbox.large.admit(length).is_none());
        assert!(body.bytes().is_empty(), "the package allocation stays off frame intake");
        let envelope = inbox.envelope(head, body, peer);
        assert!(inbox.ports.requests().submit(envelope.id(), envelope, length).is_ok());
        let mut request = inbox.take().pop().unwrap();
        assert_eq!(parse_head(&request.head).route, "/update");
        let job = request.body.take_upload().unwrap();
        assert_eq!(job.len(), length);
        assert_eq!(job.header().len(), pkg::MAX_HEADER);
        pkg::verify_header(job.header(), &keys).unwrap();
        let caller = std::thread::current().id();
        job.run(move |bytes| {
            assert_ne!(std::thread::current().id(), caller);
            assert_eq!(bytes, package);
            pkg::verify(bytes, &keys).map(|_| ())
        })
        .unwrap()
        .join()
        .unwrap()
        .unwrap();
        drop(slot);
        assert!(inbox.large.admit(length).is_some());
    });
}
