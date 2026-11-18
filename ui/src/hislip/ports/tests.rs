// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;
use crate::netio::port::MAX_CLIENTS;

fn ports() -> Arc<Ports> {
    Ports::bounded(Arc::new(Wake::new().unwrap()), MAX_CLIENTS, MAX_CLIENTS * REPLY_BYTES).unwrap()
}
fn submit(ports: &Ports, connection: u64, sequence: u64, typ: u8, param: u32, data: &[u8]) {
    let request = Envelope::message(
        connection,
        sequence,
        RemoteRequest::Hislip {
            message: Some(Message { typ, control: 0, param, data: data.to_vec() }),
            closed: Default::default(),
            cleared: Default::default(),
        },
    );
    assert!(ports.requests().submit(request.id(), request, data.len()).is_ok());
}
fn drain(server: &mut Server, ports: &Ports) -> Vec<crate::netio::port::Reply> {
    ports.flush(&mut server.parked);
    ports.replies().take(MAX_CLIENTS)
}

#[test]
fn injected_intake_time_yields_without_executing_or_replaying_commands() {
    let mut server = Server::closed();
    let ports = ports();
    submit(&ports, 1, 1, INITIALIZE, 0x0100_0000, b"hislip0");
    let mut calls = 0;
    let mut exec = |p: &mut crate::scpi::Program| {
        calls += 1;
        Some(p.immediate(&mut |_| b"answer".to_vec()))
    };
    server.poll_ports(&ports, || Duration::from_micros(500), &mut exec);
    assert!(!ports.requests().is_empty());
    server.poll_ports(&ports, || Duration::ZERO, &mut exec);
    drain(&mut server, &ports);
    submit(&ports, 1, 2, DATA_END, 42, b"X?");
    server.poll_ports(&ports, || Duration::ZERO, &mut exec);
    let replies = drain(&mut server, &ports);
    assert_eq!(replies.len(), 1);
    assert_eq!(replies[0].output().chunks().next().unwrap(), b"answer\n");
    server.poll_ports(&ports, || Duration::ZERO, &mut exec);
    assert_eq!(calls, 1);
    drop(replies);
    assert_eq!(ports.budget().used(), 0);
}

#[test]
fn disconnect_discards_pending_tail_and_releases_its_reservation() {
    let mut server = Server::closed();
    let ports = ports();
    submit(&ports, 1, 1, INITIALIZE, 0x0100_0000, b"hislip0");
    server.poll_ports(&ports, || Duration::ZERO, &mut |_| panic!("handshake is not a program"));
    drain(&mut server, &ports);
    submit(&ports, 1, 2, DATA_END, 42, b"*OPC?;:RUN");
    server.poll_ports(&ports, || Duration::ZERO, &mut |_| None);
    assert!(ports.budget().used() >= REPLY_BYTES);
    server.conns[0].closed.store(true, Ordering::Release);
    server.poll_ports(&ports, || Duration::ZERO, &mut |_| panic!("disconnected tail must not run"));
    assert!(server.live.is_empty());
    assert!(server.conns.is_empty());
    assert_eq!(ports.budget().used(), 0);
}

#[test]
fn backpressure_parks_owned_replies_without_replaying_programs() {
    let mut server = Server::closed();
    let ports = ports();
    for id in 1..=2 {
        submit(&ports, id, 1, INITIALIZE, 0x0100_0000, b"hislip0");
        server.poll_ports(&ports, || Duration::ZERO, &mut |_| panic!("handshake"));
        drain(&mut server, &ports);
    }
    let mut calls = 0;
    for id in 1..=2 {
        submit(&ports, id, 2, DATA_END, 42, b"X?");
    }
    let mut exec = |p: &mut crate::scpi::Program| {
        calls += 1;
        Some(p.immediate(&mut |_| b"answer".to_vec()))
    };
    server.poll_ports(&ports, || Duration::ZERO, &mut exec);
    server.poll_ports(&ports, || Duration::ZERO, &mut exec);
    assert!(server.parked.is_some());
    assert_eq!(ports.replies().take(1).len(), 1);
    server.poll_ports(&ports, || Duration::ZERO, &mut exec);
    assert_eq!(drain(&mut server, &ports).len(), 1);
    assert_eq!(calls, 2);
    assert_eq!(ports.budget().used(), 0);
}

#[test]
fn all_eight_pending_connections_get_an_intake_turn() {
    let mut server = Server::closed();
    let ports = ports();
    for id in 1..=8 {
        submit(&ports, id, 1, INITIALIZE, 0x0100_0000, b"hislip0");
        server.poll_ports(&ports, || Duration::ZERO, &mut |_| panic!("handshake"));
        drain(&mut server, &ports);
    }
    for id in 1..=8 {
        submit(&ports, id, 2, DATA_END, 42, b"*OPC?");
    }
    let mut calls = 0;
    server.poll_ports(&ports, || Duration::ZERO, &mut |_| {
        calls += 1;
        None
    });
    assert_eq!(calls, 8);
    server.poll_ports(&ports, || Duration::ZERO, &mut |_| {
        calls += 1;
        None
    });
    assert_eq!(calls, 16);
    assert_eq!(server.conns.iter().filter(|c| c.active.is_some()).count(), 8);
}
