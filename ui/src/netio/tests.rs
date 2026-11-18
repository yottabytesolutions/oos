// SPDX-License-Identifier: GPL-2.0-or-later
use super::port::{RemoteRequest, RESPONSE_BYTES};
use super::*;
use std::{io::Write, net::TcpStream};

fn next_request(ports: &Ports) -> Envelope {
    crate::ui::lifecycle::test_wait::until("raw IO admission", || ports.requests().take(1).pop())
}

#[test]
fn connection_credit_covers_output_and_shutdown_cancels_owned_requests() {
    let wake = Arc::new(Wake::new().unwrap());
    let raw =
        Service::start(crate::net6::Port::Host, false, wake, Runtime::new().unwrap(), crate::netio::Protocol::Raw)
            .unwrap();
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
    assert_eq!(second.id(), super::port::Id::new(first.id().connection(), 2));
    assert!(matches!(second.request(), RemoteRequest::Program(text) if text == "second\n"));
    drop(raw);
    assert!(second.cancelled(), "shutdown cancels the IO-held credit before joining the thread");
    assert!(ports.requests().is_empty());
    assert_eq!(ports.budget().used(), 0);
}

#[test]
fn rpc_disconnect_cancels_an_admitted_request_before_a_frame_reply() {
    let wake = Arc::new(Wake::new().unwrap());
    let runtime = Runtime::new().unwrap();
    let io = Service::start(
        crate::net6::Port::Host,
        false,
        wake,
        runtime,
        crate::netio::Protocol::Rpc(super::port::rpc::Channel::Core),
    )
    .unwrap();
    io.set_acl(crate::netacl::Shared::default());
    let ports = io.ports();
    let mut client = TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, io.port())).unwrap();
    let words = [1u32, 0, 2, super::port::rpc::CORE_PROG, 1, 0, 0, 0, 0, 0];
    client.write_all(&(40u32 | 1 << 31).to_be_bytes()).unwrap();
    for word in words {
        client.write_all(&word.to_be_bytes()).unwrap();
    }
    let request = next_request(&ports);
    let RemoteRequest::Vxi11 { closed, .. } = request.request() else { panic!("RPC request expected") };
    drop(client);
    let guard = crate::ui::lifecycle::test_wait::Wait::new("RPC cancellation");
    while !closed.load(std::sync::atomic::Ordering::Acquire) {
        guard.check();
        std::thread::yield_now();
    }
    assert!(request.cancelled());
    assert!(ports.requests().is_empty());
    assert_eq!(ports.budget().used(), 0);
}

#[test]
fn rpc_request_queue_reserves_a_udp_slot_beside_the_eight_tcp_slots() {
    let io = Service::start(
        crate::net6::Port::Host,
        false,
        Arc::new(Wake::new().unwrap()),
        Runtime::new().unwrap(),
        crate::netio::Protocol::Rpc(super::port::rpc::Channel::Core),
    )
    .unwrap();
    let ports = io.ports();
    for connection in 0..=MAX_CLIENTS {
        let request = Envelope::message(
            connection as u64,
            1,
            RemoteRequest::Vxi11 {
                record: vec![],
                channel: super::port::rpc::Channel::Core,
                closed: Default::default(),
            },
        );
        assert!(ports.requests().submit(request.id(), request, 0).is_ok());
    }
    let overflow = Envelope::program(9, 1, String::new());
    assert!(ports.requests().submit(overflow.id(), overflow, 0).is_err());
    assert_eq!(ports.requests().take(MAX_CLIENTS + 1).len(), MAX_CLIENTS + 1);
}

#[test]
fn hislip_fragmentation_clear_and_disconnect_cancel_the_original_credit() {
    use super::port::hislip::*;
    let io = Service::start(
        crate::net6::Port::Host,
        false,
        Arc::new(Wake::new().unwrap()),
        Runtime::new().unwrap(),
        Protocol::Hislip,
    )
    .unwrap();
    io.set_acl(crate::netacl::Shared::default());
    let ports = io.ports();
    let mut client = TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, io.port())).unwrap();
    let message = encode(DATA_END, 0, 42, b"*OPC?");
    client.write_all(&message[..3]).unwrap();
    assert!(ports.requests().is_empty());
    client.write_all(&message[3..]).unwrap();
    let request = next_request(&ports);
    assert!(matches!(request.request(), RemoteRequest::Hislip { message: Some(m), .. } if m.param == 42));
    client.write_all(&encode(DEVICE_CLEAR_COMPLETE, 0, 0, b"")).unwrap();
    let clear = next_request(&ports);
    assert!(request.cancelled());
    assert!(
        matches!(clear.request(), RemoteRequest::Hislip { message: Some(m), .. } if m.typ == DEVICE_CLEAR_COMPLETE)
    );
    drop(client);
    crate::ui::lifecycle::test_wait::until("HiSLIP disconnect cancellation", || clear.cancelled().then_some(()));
    assert_eq!(ports.budget().used(), 0);
}
#[test]
fn host_listeners_are_private_and_independent() {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let _enter = rt.enter();
    let a = runtime::listen(crate::net6::Port::Host, true).unwrap();
    let b = runtime::listen(crate::net6::Port::Host, false).unwrap();
    let (a, b) = (a.local_addr().unwrap(), b.local_addr().unwrap());
    assert_eq!(a.ip(), core::net::Ipv4Addr::LOCALHOST);
    assert_eq!(b.ip(), core::net::Ipv4Addr::LOCALHOST);
    assert_ne!(a.port(), 0);
    assert_ne!(a.port(), b.port());
}

#[test]
fn service_listeners_keep_dual_stack_or_ipv4_fallback() {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let _enter = rt.enter();
    let dual = runtime::listen(crate::net6::Port::Service(0), true).unwrap();
    assert!(dual.local_addr().unwrap().ip().is_unspecified());
    let ipv4 = runtime::listen(crate::net6::Port::Service(0), false).unwrap();
    assert!(ipv4.local_addr().unwrap().is_ipv4());
}
