// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;
use std::{io::Write, net::TcpStream};

fn next_request(ports: &Ports) -> Envelope {
    crate::ui::lifecycle::test_wait::until("raw IO admission", || ports.requests().take(1).pop())
}

#[test]
fn connection_credit_covers_output_and_shutdown_cancels_owned_requests() {
    let wake = Arc::new(Wake::new().unwrap());
    let raw = Raw::open(crate::net6::Port::Host, false, wake).unwrap();
    raw.set_acl(crate::netacl::Shared::default());
    let ports = raw.ports();
    let mut client = TcpStream::connect(("127.0.0.1", raw.port())).unwrap();
    client.write_all(b"fir").unwrap();
    assert!(ports.requests().is_empty(), "a fragment is not an application message");
    client.write_all(b"st\nsecond\n").unwrap();
    let first = next_request(&ports);
    assert!(matches!(first.request(), RemoteRequest::Program(text) if text == "first\n"));
    assert!(ports.requests().is_empty(), "the pipelined program must wait for the credit");
    let reply = Reply::new(first.id(), Vec::new(), ports.budget().reserve(RESPONSE_BYTES).unwrap());
    assert!(ports.replies().submit(reply.id(), reply, 0).is_ok());
    let second = next_request(&ports);
    assert_eq!(second.id(), crate::netport::Id::new(first.id().connection(), 2));
    assert!(matches!(second.request(), RemoteRequest::Program(text) if text == "second\n"));
    drop(raw);
    assert!(second.cancelled(), "shutdown cancels the IO-held credit before joining the thread");
    assert!(ports.requests().is_empty());
    assert_eq!(ports.budget().used(), 0);
}
