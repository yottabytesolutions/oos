// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;
#[test]
fn memory_reset_releases_replies_and_shutdown_releases_links() {
    let mut server = Server::closed(crate::netio::Runtime::new().unwrap(), Arc::new(Wake::new().unwrap()));
    server.links.push(Some(Link { out: vec![42; 4096], ..Default::default() }));
    server.release_buffers();
    assert_eq!(server.links.len(), 1);
    assert_eq!(server.links[0].as_ref().unwrap().out.capacity(), 0);
    server.set_open(false, false);
    assert!(server.links.is_empty());
}

use std::{
    io::{Read, Write},
    net::{Ipv4Addr, TcpStream},
};

fn server() -> Server {
    let mut server = Server::host(crate::netio::Runtime::new().unwrap(), Arc::new(Wake::new().unwrap())).unwrap();
    server.poll_async(&mut |_| None);
    server
}
fn call(socket: &mut TcpStream, procedure: u32, args: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for word in [procedure, 0, 2, CORE_PROG, 1, procedure, 0, 0, 0, 0] {
        put(&mut bytes, word);
    }
    bytes.extend_from_slice(args);
    // Two fragments, coalesced into one write, exercise the IO record decoder.
    let mut record = 8u32.to_be_bytes().to_vec();
    record.extend_from_slice(&bytes[..8]);
    record.extend_from_slice(&((bytes.len() as u32 - 8) | 1 << 31).to_be_bytes());
    record.extend_from_slice(&bytes[8..]);
    socket.write_all(&record).unwrap();
    let mut header = [0; 4];
    socket.read_exact(&mut header).unwrap();
    let mut reply = vec![0; (u32::from_be_bytes(header) & 0x7fffffff) as usize];
    socket.read_exact(&mut reply).unwrap();
    assert_eq!(Xdr::new(&reply).u32(), Some(procedure));
    reply
}
fn args(words: &[u32], data: Option<&[u8]>) -> Vec<u8> {
    let mut bytes = Vec::new();
    for &word in words {
        put(&mut bytes, word);
    }
    if let Some(data) = data {
        put_opaque(&mut bytes, data);
    }
    bytes
}
fn drive(
    server: &mut Server,
    exec: &mut dyn FnMut(&mut crate::scpi::Program) -> Option<Vec<u8>>,
    done: impl Fn() -> bool,
) {
    let guard = crate::ui::lifecycle::test_wait::Wait::new("VXI IO completion");
    while !done() {
        guard.check();
        server.poll_async(exec);
        std::thread::yield_now();
    }
}
fn connect(server: &Server) -> TcpStream {
    let socket = TcpStream::connect((Ipv4Addr::LOCALHOST, server.service_port().unwrap())).unwrap();
    socket.set_read_timeout(Some(crate::ui::lifecycle::test_wait::HANG_GUARD)).unwrap();
    socket
}

#[test]
fn rpc_io_preserves_pending_program_limits_and_disconnect_releases_links() {
    let mut server = server();
    let mut socket = connect(&server);
    let client = std::thread::spawn(move || {
        for lid in 0..16 {
            let reply = call(&mut socket, 10, &[]);
            let mut body = Xdr::new(&reply[24..]);
            assert_eq!((body.u32(), body.u32()), (Some(0), Some(lid)));
            assert_eq!((body.u32(), body.u32()), (Some(0), Some(1 << 20)));
        }
        assert_eq!(Xdr::new(&call(&mut socket, 10, &[])[24..]).u32(), Some(9));
        for _ in 0..16 {
            let reply = call(&mut socket, 11, &args(&[0, 1000, 0, 8], Some(b"*OPC?")));
            assert_eq!(Xdr::new(&reply[24..]).u32(), Some(0));
        }
        let reply = call(&mut socket, 11, &args(&[0, 1000, 0, 8], Some(b"*OPC?")));
        assert_eq!(Xdr::new(&reply[24..]).u32(), Some(9));
        let reply = call(&mut socket, 12, &args(&[0, 1024, 1000, 0, 0, 0], None));
        assert_eq!(Xdr::new(&reply[24..]).u32(), Some(15));
    });
    drive(&mut server, &mut |_| None, || client.is_finished());
    client.join().unwrap();
    let guard = crate::ui::lifecycle::test_wait::Wait::new("disconnected links released");
    while server.links.iter().any(Option::is_some) {
        guard.check();
        server.poll_async(&mut |_| None);
        std::thread::yield_now();
    }
    assert_eq!(server.endpoint.io.as_ref().unwrap().ports().budget().used(), 0);
}

#[test]
fn rpc_io_preserves_empty_binary_replies_and_invalid_link_errors() {
    let mut server = server();
    let mut socket = connect(&server);
    let client = std::thread::spawn(move || {
        call(&mut socket, 10, &[]);
        call(&mut socket, 11, &args(&[0, 1000, 0, 8], Some(b"EMPTY?;BINARY?")));
        let reply = call(&mut socket, 12, &args(&[0, 2, 1000, 0, 0, 0], None));
        let mut body = Xdr::new(&reply[24..]);
        assert_eq!((body.u32(), body.u32(), body.opaque()), (Some(0), Some(1), Some(&b";\0"[..])));
        let reply = call(&mut socket, 12, &args(&[0, 1024, 1000, 0, 0, 0], None));
        let mut body = Xdr::new(&reply[24..]);
        assert_eq!((body.u32(), body.u32(), body.opaque()), (Some(0), Some(4), Some(&b"\xff\n"[..])));
        call(&mut socket, 23, &args(&[0], None));
        assert_eq!(Xdr::new(&call(&mut socket, 12, &args(&[0, 1024], None))[24..]).u32(), Some(4));
    });
    drive(
        &mut server,
        &mut |p| Some(p.immediate(&mut |command| if command == "BINARY?" { vec![0, 255] } else { vec![] })),
        || client.is_finished(),
    );
    client.join().unwrap();
}

#[test]
fn oversized_unfinished_record_is_closed_by_io() {
    let mut server = server();
    let mut socket = connect(&server);
    let client = std::thread::spawn(move || {
        let mut record = (MAX_RECORD as u32 | 1 << 31).to_be_bytes().to_vec();
        record.resize(MAX_RECORD, 0);
        socket.write_all(&record).unwrap();
        let mut probe = [0];
        assert_eq!(socket.read(&mut probe).unwrap(), 0);
    });
    drive(&mut server, &mut |_| None, || client.is_finished());
    client.join().unwrap();
    assert!(server.links.is_empty());
}

#[test]
fn a_read_larger_than_one_program_keeps_the_existing_payload_limit() {
    let mut server = server();
    let n = crate::scpi::RESP_MAX + 1;
    server.links.push(Some(Link { out: vec![0x81; n], ..Default::default() }));
    let ports = server.endpoint.io.as_ref().unwrap().ports();
    let mut socket = connect(&server);
    let client = std::thread::spawn(move || {
        let reply = call(&mut socket, 12, &args(&[0, n as u32], None));
        let mut body = Xdr::new(&reply[24..]);
        assert_eq!((body.u32(), body.u32()), (Some(0), Some(4)));
        let bytes = body.opaque().unwrap();
        assert_eq!(bytes.len(), n);
        assert!(bytes.iter().all(|&b| b == 0x81));
    });
    drive(&mut server, &mut |_| None, || client.is_finished());
    client.join().unwrap();
    let guard = crate::ui::lifecycle::test_wait::Wait::new("RPC reservation returned");
    while ports.budget().used() != 0 {
        guard.check();
        std::thread::yield_now();
    }
}

#[test]
fn closing_raw_scpi_keeps_vxi_on_the_shared_runtime() {
    let mut server = server();
    let raw = crate::netio::Service::start(
        crate::net6::Port::Host,
        false,
        server.endpoint.wake.clone(),
        server.endpoint.runtime().unwrap(),
        crate::netio::Protocol::Raw,
    )
    .unwrap();
    drop(raw);
    let mut socket = connect(&server);
    let client = std::thread::spawn(move || {
        assert_eq!(call(&mut socket, 0, &[]).len(), 24);
    });
    drive(&mut server, &mut |_| None, || client.is_finished());
    client.join().unwrap();
}

fn request(sequence: u64, procedure: u32, args: &[u8]) -> Envelope {
    let mut record = Vec::new();
    for word in [sequence as u32, 0, 2, CORE_PROG, 1, procedure, 0, 0, 0, 0] {
        put(&mut record, word);
    }
    record.extend_from_slice(args);
    Envelope::message(1, sequence, RemoteRequest::Vxi11 { record, channel: Channel::Core, closed: Default::default() })
}

#[test]
fn device_read_waits_for_pixels_within_its_requested_timeout() {
    let mut server = Server::closed(crate::netio::Runtime::new().unwrap(), Arc::new(Wake::new().unwrap()));
    server.links.push(Some(Link::default()));
    let ports = Ports::bounded(server.endpoint.wake.clone(), crate::netport::MAX_CLIENTS, REPLY_BYTES).unwrap();
    let mut ui = crate::Ui::for_tests(crate::Source::Demo { t: 0.0 });
    let mut scpi = crate::scpi::Scpi::disconnected();
    scpi.use_readback(Default::default());
    let cv = crate::gfx::Canvas::new(2, 2);
    let start = Instant::now();
    for (index, ready) in [true, false].into_iter().enumerate() {
        let now = start + Duration::from_secs(index as u64);
        let sequence = index as u64 * 2;
        let write = request(sequence + 1, 11, &args(&[0, 1000, 0, 8], Some(b":DISP:DATA?")));
        let read = request(sequence + 2, 12, &args(&[0, 1024, 100, 0, 0, 0], None));
        for rpc in [write, read] {
            ports.requests().submit(rpc.id(), rpc, 64).ok().unwrap();
        }
        server.poll(&ports, now, || Duration::ZERO, &mut |p| scpi.poll_program(&mut ui, &cv, p));
        assert_eq!(ports.replies().take(2).len(), 1, "only device_write has answered");
        assert_eq!(server.screen_deadline(now), Some(now + Duration::from_millis(100)));
        if ready {
            let demand = scpi.readback().unwrap();
            demand.take().unwrap().complete(&cv);
        }
        let elapsed = Duration::from_millis(if ready { 25 } else { 100 });
        server.poll(&ports, now + elapsed, || Duration::ZERO, &mut |p| scpi.poll_program(&mut ui, &cv, p));
        server.poll(&ports, now + elapsed, || Duration::ZERO, &mut |p| scpi.poll_program(&mut ui, &cv, p));
        let replies = ports.replies().take(2);
        assert_eq!(replies.len(), 1);
        assert_eq!(replies[0].id().request(), sequence + 2);
        let reply: Vec<u8> = replies[0].output().chunks().flatten().copied().collect();
        let mut payload = Xdr::new(&reply[28..]); // RPC record header and accept reply.
        assert_eq!(payload.u32(), Some(if ready { 0 } else { 15 }));
        assert_eq!(payload.u32(), Some(if ready { 4 } else { 0 }));
        let bytes = payload.opaque().unwrap();
        if ready {
            assert_eq!(&bytes[11..13], b"BM");
        } else {
            assert!(bytes.is_empty());
        }
    }
}

#[test]
fn reply_backpressure_and_injected_deadline_never_repeat_a_setter() {
    let mut server = Server::closed(crate::netio::Runtime::new().unwrap(), Arc::new(Wake::new().unwrap()));
    server.links.push(Some(Link::default()));
    let ports = Ports::bounded(server.endpoint.wake.clone(), crate::netport::MAX_CLIENTS + 1, REPLY_BYTES).unwrap();
    let occupied = Reply::new(crate::netport::Id::new(2, 1), vec![], ports.budget().reserve(256).unwrap());
    ports.replies().submit(occupied.id(), occupied, 0).ok().unwrap();
    let request = request(1, 11, &args(&[0, 1000, 0, 8], Some(b"SETTER")));
    ports.requests().submit(request.id(), request, 64).ok().unwrap();
    let mut executions = 0;
    let mut exec = |p: &mut crate::scpi::Program| {
        Some(p.immediate(&mut |_| {
            executions += 1;
            vec![]
        }))
    };
    server.poll(&ports, Instant::now(), || std::time::Duration::from_micros(500), &mut exec);
    assert!(!ports.requests().is_empty());
    server.poll(&ports, Instant::now(), || std::time::Duration::ZERO, &mut exec);
    assert!(server.parked.is_some());
    server.poll(&ports, Instant::now(), || std::time::Duration::ZERO, &mut exec);
    assert!(server.parked.is_some());
    drop(ports.replies().take(1));
    server.poll(&ports, Instant::now(), || std::time::Duration::ZERO, &mut exec);
    assert!(server.parked.is_none());
    assert_eq!(executions, 1);
    assert_eq!(ports.replies().take(1)[0].id().request(), 1);
    assert_eq!(ports.budget().used(), 0);
}

#[test]
fn exhausted_reply_budget_defers_interpretation_and_cancellation_releases_it() {
    let mut server = Server::closed(crate::netio::Runtime::new().unwrap(), Arc::new(Wake::new().unwrap()));
    let ports = Ports::bounded(server.endpoint.wake.clone(), crate::netport::MAX_CLIENTS + 1, REPLY_BYTES).unwrap();
    let held = ports.budget().reserve(REPLY_BYTES).unwrap();
    let request = request(1, 10, &[]);
    let credit = request.credit(ports.clone());
    ports.requests().submit(request.id(), request, 40).ok().unwrap();
    server.poll(&ports, Instant::now(), || std::time::Duration::ZERO, &mut |_| panic!("no SCPI expected"));
    assert!(server.pending.is_some());
    assert!(server.links.is_empty(), "a resource wait must not create a link");
    drop(credit);
    drop(held);
    server.poll(&ports, Instant::now(), || std::time::Duration::ZERO, &mut |_| panic!("cancelled request"));
    assert!(server.pending.is_none());
    assert!(server.links.is_empty());
    assert!(ports.replies().is_empty());
    assert_eq!(ports.budget().used(), 0);
}

#[test]
fn stopping_vxi_cancels_its_queued_request_without_stopping_raw_scpi() {
    let mut server = server();
    let raw = crate::netio::Service::start(
        crate::net6::Port::Host,
        false,
        server.endpoint.wake.clone(),
        server.endpoint.runtime().unwrap(),
        crate::netio::Protocol::Raw,
    )
    .unwrap();
    raw.set_acl(crate::netacl::Shared::default());
    let ports = server.endpoint.io.as_ref().unwrap().ports();
    let mut client = connect(&server);
    client.write_all(&(40u32 | 1 << 31).to_be_bytes()).unwrap();
    if let RemoteRequest::Vxi11 { record, .. } = request(1, 10, &[]).request() {
        client.write_all(record).unwrap();
    }
    let queued =
        crate::ui::lifecycle::test_wait::until("RPC queued before shutdown", || ports.requests().take(1).pop());
    server.set_open(false, false);
    assert!(queued.cancelled());
    assert!(ports.requests().is_empty());
    assert!(ports.replies().is_empty());
    let mut client = TcpStream::connect((Ipv4Addr::LOCALHOST, raw.port())).unwrap();
    client.write_all(b"*IDN?\n").unwrap();
    let request =
        crate::ui::lifecycle::test_wait::until("raw survives VXI shutdown", || raw.ports().requests().take(1).pop());
    assert!(matches!(request.request(), RemoteRequest::Program(p) if p == "*IDN?\n"));
}

#[test]
fn malformed_rpc_completes_without_reply_and_the_next_call_still_runs() {
    let mut server = server();
    let mut socket = connect(&server);
    let client = std::thread::spawn(move || {
        socket.write_all(&(3u32 | 1 << 31).to_be_bytes()).unwrap();
        socket.write_all(&[0xff; 3]).unwrap();
        assert_eq!(call(&mut socket, 0, &[]).len(), 24);
    });
    drive(&mut server, &mut |_| panic!("RPC NULL does not interpret SCPI"), || client.is_finished());
    client.join().unwrap();
}
