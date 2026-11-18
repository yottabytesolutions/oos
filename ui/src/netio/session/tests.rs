// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;
use std::io::Write;

#[test]
fn withdrawing_policy_keeps_an_established_session_usable() {
    use std::{
        future::Future,
        task::{Context, Waker},
    };
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
        let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let mut client = TcpStream::connect(listener.local_addr().unwrap()).await.unwrap();
        let (socket, _) = listener.accept().await.unwrap();
        let ports = Ports::new(Arc::new(Wake::new().unwrap())).unwrap();
        let (send, replies) = mpsc::channel(1);
        let (control, acl) = watch::channel(Some(crate::netacl::Shared::default()));
        let mut session = std::pin::pin!(run(1, socket, ports.clone(), replies, acl, Protocol::Raw));
        let mut cx = Context::from_waker(Waker::noop());
        assert!(session.as_mut().poll(&mut cx).is_pending());
        control.send_replace(None);
        assert!(session.as_mut().poll(&mut cx).is_pending(), "withdrawal only gates listeners");
        let exchange = async {
            client.write_all(b"*IDN?\n").await.unwrap();
            ports.requests().notified().await;
            let request = ports.requests().take(1).pop().unwrap();
            assert!(matches!(request.request(), RemoteRequest::Program(text) if text == "*IDN?\n"));
            let reply = Reply::new(request.id(), b"OOS\n".to_vec(), ports.budget().reserve_available().unwrap());
            send.send(reply).await.unwrap();
            let mut response = [0; 4];
            client.read_exact(&mut response).await.unwrap();
            assert_eq!(&response, b"OOS\n");
        };
        tokio::time::timeout(crate::ui::lifecycle::test_wait::HANG_GUARD, async {
            tokio::select! {
                _ = &mut session => panic!("established session must stay open"),
                _ = exchange => {},
            }
        })
        .await
        .expect("session exchange must complete");
    });
}

#[test]
fn hislip_clear_discards_only_bytes_buffered_before_the_next_read() {
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
        let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let mut client = std::net::TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (socket, _) = listener.accept().await.unwrap();
        let mut buffer = b"discard this pre-clear program".to_vec();
        let cleared = std::sync::atomic::AtomicBool::new(true);
        let message = super::super::port::hislip::encode(7, 0, 44, b"*IDN?\n");
        client.write_all(&message).unwrap();
        assert!(read(&socket, &mut buffer, MAX_PROGRAM, &cleared).await.unwrap());
        assert!(!cleared.load(Ordering::Acquire));
        while buffer.len() < message.len() {
            assert!(read(&socket, &mut buffer, MAX_PROGRAM, &cleared).await.unwrap());
        }
        assert_eq!(buffer, message, "a fresh command after the clear acknowledgement must survive");
        assert!(!cleared.load(Ordering::Acquire));
    });
}

#[test]
fn hislip_read_ahead_overflow_cancels_instead_of_holding_a_credit_forever() {
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
        let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let mut client = std::net::TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (socket, _) = listener.accept().await.unwrap();
        let mut buffer = vec![0; MAX_PROGRAM + super::super::port::hislip::HEADER];
        buffer[..2].copy_from_slice(b"HS");
        buffer[2] = super::super::port::hislip::DATA_END;
        client.write_all(b"x").unwrap();
        let cleared = std::sync::atomic::AtomicBool::new(false);
        let result = tokio::time::timeout(
            crate::ui::lifecycle::test_wait::HANG_GUARD,
            read_clear(&socket, &mut buffer, &cleared),
        )
        .await
        .expect("read-ahead overflow must complete");
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::InvalidData);
    });
}
