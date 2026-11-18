// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;
use std::{
    future::Future,
    task::{Context, Waker},
};

#[test]
fn withdrawing_policy_leaves_connections_in_the_backlog_until_reenabled() {
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
        let listener = listen(crate::net6::Port::Host, false).unwrap();
        let (control, changed) = watch::channel(Some(crate::netacl::Shared::default()));
        let mapper = None;
        let mut work = std::pin::pin!(accept(&listener, &mapper, Protocol::Raw, changed));
        let mut cx = Context::from_waker(Waker::noop());
        assert!(work.as_mut().poll(&mut cx).is_pending());
        control.send_replace(None);
        let readiness = std::net::TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let client = std::net::TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (_, peer) = listener.accept().await.unwrap();
        assert_eq!(peer, readiness.local_addr().unwrap());
        assert!(work.as_mut().poll(&mut cx).is_pending());
        control.send_replace(Some(crate::netacl::Shared::default()));
        let (socket, peer, _) = work.await.unwrap();
        assert_eq!(peer, client.local_addr().unwrap());
        assert_eq!(socket.local_addr().unwrap(), client.peer_addr().unwrap());
    });
}

#[test]
fn routing_replies_preserves_pending_frame_notifications() {
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
        let ports = Ports::new(Arc::new(Wake::new().unwrap())).unwrap();
        let (send, mut receive) = mpsc::channel(1);
        let sessions = std::sync::Mutex::new(HashMap::from([(1, send)]));
        let request = Envelope::program(1, 1, "query?".into());
        let id = request.id();
        assert!(ports.requests().submit(id, request, 0).is_ok());
        let reply = Reply::new(id, Vec::new(), ports.budget().reserve(0).unwrap());
        assert!(ports.replies().submit(id, reply, 0).is_ok());
        let mut work = std::pin::pin!(route(&ports, &sessions));
        assert!(work.as_mut().poll(&mut Context::from_waker(Waker::noop())).is_pending());
        assert_eq!(receive.try_recv().unwrap().id(), id);
        assert!(ports.requests().wake().wait(0));
        assert_eq!(ports.requests().take(1).pop().unwrap().id(), id);
    });
}
