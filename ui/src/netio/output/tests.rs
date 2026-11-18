// SPDX-License-Identifier: GPL-2.0-or-later
use super::super::port::{Budget, Id, Wake, RESPONSE_BYTES};
use super::*;
use std::{
    cell::Cell,
    pin::Pin,
    rc::Rc,
    sync::Arc,
    task::{Context, Poll, Waker},
};

struct Writer {
    bytes: Vec<u8>,
    step: Duration,
    now: Rc<Cell<Duration>>,
    stalled: bool,
}
impl AsyncWrite for Writer {
    fn poll_write(mut self: Pin<&mut Self>, _: &mut Context<'_>, bytes: &[u8]) -> Poll<io::Result<usize>> {
        if self.stalled {
            return Poll::Pending;
        }
        let n = bytes.len().min(2);
        self.bytes.extend_from_slice(&bytes[..n]);
        self.now.set(self.now.get() + self.step);
        Poll::Ready(Ok(n))
    }
    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
    fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

#[test]
fn partial_writes_and_both_timeouts_release_owned_output() {
    for framing in 0..5 {
        for (step, stalled, timeout) in [
            (Duration::from_millis(20), false, false),
            (Duration::from_secs(1), false, true),
            (Duration::ZERO, true, true),
        ] {
            let budget = Budget::new(Arc::new(Wake::new().unwrap()));
            let now = Rc::new(Cell::new(Duration::ZERO));
            let mut writer = Writer { bytes: Vec::new(), step, now: now.clone(), stalled };
            let bytes = vec![b'x'; 64];
            let mut plain = super::super::port::Output::default();
            assert!(plain.push(bytes.clone()).is_ok());
            let output = match framing {
                0 => plain,
                1 => super::super::port::Output::rpc(bytes),
                2 => super::super::port::Output::hislip(bytes, 77, 16),
                3 => super::super::port::Output::http("HTTP/1.1 200 OK\r\n\r\n".into(), bytes.into()),
                _ => {
                    super::super::port::Output::http("HTTP/1.1 200 OK\r\n\r\n".into(), Arc::<[u8]>::from(bytes).into())
                }
            };
            let reply = Reply::from_output(Id::new(1, 1), output, budget.reserve(RESPONSE_BYTES).unwrap());
            let request = super::super::port::Envelope::program(1, 1, "query?".into());
            let ports = super::super::port::Ports::new(Arc::new(Wake::new().unwrap())).unwrap();
            let send = async {
                let _credit = request.credit(ports);
                send(
                    &mut writer,
                    reply,
                    || now.get(),
                    move |_| std::future::poll_fn(move |_| if stalled { Poll::Ready(()) } else { Poll::Pending }),
                )
                .await
            };
            let mut send = std::pin::pin!(send);
            let Poll::Ready(result) = send.as_mut().poll(&mut Context::from_waker(Waker::noop())) else {
                panic!("fixture must complete");
            };
            assert_eq!(result.as_ref().err().map(io::Error::kind), timeout.then_some(io::ErrorKind::TimedOut));
            assert_eq!(budget.used(), 0, "completion and timeout release response capacity");
            assert!(request.cancelled(), "completion and both timeout paths return the connection credit");
        }
    }
}

#[test]
fn cancelling_a_stalled_write_releases_its_reservation() {
    let budget = Budget::new(Arc::new(Wake::new().unwrap()));
    let mut writer =
        Writer { bytes: Vec::new(), step: Duration::ZERO, now: Rc::new(Cell::new(Duration::ZERO)), stalled: true };
    for output in [
        super::super::port::Output::hislip(vec![0; 4096], 77, 4096),
        super::super::port::Output::http("HTTP/1.1 200 OK\r\n\r\n".into(), Arc::<[u8]>::from(vec![0; 4096]).into()),
    ] {
        let reply = Reply::from_output(Id::new(1, 1), output, budget.reserve(RESPONSE_BYTES).unwrap());
        let mut output = Box::pin(send(&mut writer, reply, || Duration::ZERO, |_| std::future::pending()));
        assert!(output.as_mut().poll(&mut Context::from_waker(Waker::noop())).is_pending());
        assert!(budget.used() >= 4096);
        drop(output);
        assert_eq!(budget.used(), 0);
    }
}

#[test]
fn stalled_writer_does_not_delay_a_healthy_connections_output() {
    let ports = super::super::port::Ports::new(Arc::new(Wake::new().unwrap())).unwrap();
    let clock = Rc::new(Cell::new(Duration::ZERO));
    let mut stalled = Writer { bytes: Vec::new(), step: Duration::ZERO, now: clock.clone(), stalled: true };
    let held = Reply::from_output(
        Id::new(1, 1),
        super::super::port::Output::hislip(vec![0; 4096], 77, 4096),
        ports.budget().reserve(RESPONSE_BYTES).unwrap(),
    );
    let mut output = Box::pin(send(&mut stalled, held, || clock.get(), |_| std::future::pending()));
    let mut cx = Context::from_waker(Waker::noop());
    assert!(output.as_mut().poll(&mut cx).is_pending());
    let mut healthy =
        Writer { bytes: Vec::new(), step: Duration::from_micros(100), now: clock.clone(), stalled: false };
    let reply = Reply::from_output(
        Id::new(2, 1),
        super::super::port::Output::hislip(b"ready\n".to_vec(), 78, 4096),
        ports.budget().reserve(1024).unwrap(),
    );
    {
        let send = send(&mut healthy, reply, || clock.get(), |_| std::future::pending());
        let mut send = std::pin::pin!(send);
        assert!(matches!(send.as_mut().poll(&mut cx), Poll::Ready(Ok(()))));
    }
    assert_eq!(healthy.bytes, super::super::port::hislip::encode(7, 0, 78, b"ready\n"));
    assert!(clock.get() <= Duration::from_millis(33));
    assert!(output.as_mut().poll(&mut cx).is_pending());
    drop(output);
    assert_eq!(ports.budget().used(), 0);
}

#[test]
fn hislip_partial_writes_preserve_fragment_headers_empty_replies_and_message_ids() {
    for bytes in [Vec::new(), vec![0; 4096], vec![1; 8193]] {
        let budget = Budget::new(Arc::new(Wake::new().unwrap()));
        let now = Rc::new(Cell::new(Duration::ZERO));
        let mut writer = Writer { bytes: Vec::new(), step: Duration::ZERO, now, stalled: false };
        let reply = Reply::from_output(
            Id::new(1, 1),
            super::super::port::Output::hislip(bytes.clone(), 42, 4096),
            budget.reserve(RESPONSE_BYTES).unwrap(),
        );
        let work = send(&mut writer, reply, || Duration::ZERO, |_| std::future::pending());
        {
            let mut work = std::pin::pin!(work);
            assert!(matches!(work.as_mut().poll(&mut Context::from_waker(Waker::noop())), Poll::Ready(Ok(()))));
        }
        let count = bytes.len().div_ceil(4096).max(1);
        let mut received = Vec::new();
        for index in 0..count {
            let message = super::super::port::hislip::take_message(&mut writer.bytes).unwrap().unwrap();
            assert_eq!((message.typ, message.control, message.param), (if index + 1 == count { 7 } else { 6 }, 0, 42));
            received.extend(message.data);
        }
        assert_eq!(received, bytes);
        assert!(writer.bytes.is_empty());
        assert_eq!(budget.used(), 0);
    }
}
