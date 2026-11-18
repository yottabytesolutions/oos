// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll, Waker},
    time::Duration,
};
use tokio::io::{AsyncRead, AsyncWriteExt, ReadBuf};

fn inbox() -> Arc<Inbox> {
    let inbox = Arc::new(Inbox::default());
    inbox.active.store(1, Ordering::SeqCst);
    inbox
}

#[test]
fn hyper_output_stall_and_total_deadlines_release_owned_and_shared_responses() {
    let _alone = crate::netport::http::senders_alone();
    for shared in [false, true] {
        for total in [false, true] {
            tests::runtime().block_on(output_deadline(shared, total));
        }
    }
}
async fn output_deadline(shared: bool, total: bool) {
    let inbox = inbox();
    let (mut client, mut server) = tokio::io::duplex(128);
    client.write_all(b"GET /ui-state HTTP/1.1\r\n\r\n").await.unwrap();
    let guard = crate::ui::lifecycle::test_wait::HANG_GUARD;
    let limits = Limits { idle: guard, deadline: guard * 2, take: guard };
    let mut session = Box::pin(conversation(
        &mut server,
        "192.0.2.1:1".parse().unwrap(),
        inbox.clone(),
        Default::default(),
        limits,
        Active(inbox.clone()),
    ));
    let mut cx = Context::from_waker(Waker::noop());
    std::future::poll_fn(|cx| {
        assert!(session.as_mut().poll(cx).is_pending());
        if inbox.ports.requests().is_empty() {
            Poll::Pending
        } else {
            Poll::Ready(())
        }
    })
    .await;
    tokio::time::pause();
    let req = inbox.take().pop().unwrap();
    let body = vec![42; SEND_INLINE + 1];
    let chunk = if shared { Arc::<[u8]>::from(body).into() } else { crate::netport::output::Chunk::from(body) };
    assert!(req.reply.send(response("200 OK"), chunk, false));
    assert!(session.as_mut().poll(&mut cx).is_pending());
    assert!(inbox.ports.budget().used() > SEND_INLINE);
    assert_eq!(SENDERS.load(Ordering::SeqCst), 1);
    if total {
        for _ in 1..30 {
            tokio::time::advance(Duration::from_secs(1)).await;
            let mut buffer = [0; 128];
            let mut read = ReadBuf::new(&mut buffer);
            assert!(matches!(Pin::new(&mut client).poll_read(&mut cx, &mut read), Poll::Ready(Ok(()))));
            assert!(!read.filled().is_empty());
            assert!(session.as_mut().poll(&mut cx).is_pending());
        }
        tokio::time::advance(Duration::from_secs(1)).await;
    } else {
        tokio::time::advance(Duration::from_secs(2)).await;
    }
    let Poll::Ready(Err(error)) = session.as_mut().poll(&mut cx) else { panic!("output deadline must expire") };
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    drop(session);
    assert_eq!(inbox.ports.budget().used(), 0);
    assert_eq!(SENDERS.load(Ordering::SeqCst), 0);
    assert!(req.reply.closed());
}

#[test]
fn body_deadline_answers_408_and_releases_large_admission() {
    tests::runtime().block_on(async {
        let inbox = inbox();
        let (mut client, mut server) = tokio::io::duplex(1024);
        client
            .write_all(format!("POST /update HTTP/1.1\r\nContent-Length: {}\r\n\r\n", 2 * LARGE_BODY).as_bytes())
            .await
            .unwrap();
        let guard = crate::ui::lifecycle::test_wait::HANG_GUARD;
        let limits = Limits { idle: guard, deadline: guard * 2, take: guard };
        let mut session = Box::pin(conversation(
            &mut server,
            "192.0.2.1:1".parse().unwrap(),
            inbox.clone(),
            Default::default(),
            limits,
            Active(inbox.clone()),
        ));
        let mut cx = Context::from_waker(Waker::noop());
        std::future::poll_fn(|cx| {
            assert!(session.as_mut().poll(cx).is_pending());
            if inbox.large.occupied() {
                Poll::Ready(())
            } else {
                Poll::Pending
            }
        })
        .await;
        assert!(inbox.large.occupied());
        tokio::time::pause();
        tokio::time::advance(limits.idle).await;
        session.await.unwrap();
        assert!(!inbox.large.occupied());
        assert!(inbox.ports.requests().is_empty());
        let mut buffer = [0; 1024];
        let mut read = ReadBuf::new(&mut buffer);
        assert!(matches!(Pin::new(&mut client).poll_read(&mut cx, &mut read), Poll::Ready(Ok(()))));
        assert!(read.filled().starts_with(b"HTTP/1.1 408"));
    });
}
