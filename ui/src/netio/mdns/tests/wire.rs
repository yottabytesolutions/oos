// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;
use core::net::Ipv4Addr;
use std::net::UdpSocket;

fn daemon() -> (ServiceDaemon, u16) {
    let reservation = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let port = reservation.local_addr().unwrap().port();
    drop(reservation);
    let daemon = ServiceDaemon::new_with_port(port).unwrap();
    daemon.disable_interface(IfKind::All).unwrap();
    daemon.enable_interface(IfKind::LoopbackV4).unwrap();
    daemon.disable_interface(IfKind::IPv6).unwrap();
    (daemon, port)
}

fn announce(daemon: &ServiceDaemon, info: ServiceInfo) -> DaemonEvent {
    let events = daemon.monitor().unwrap();
    daemon.register(info).unwrap();
    loop {
        let event = events.recv_timeout(crate::ui::lifecycle::test_wait::HANG_GUARD).unwrap();
        if matches!(event, DaemonEvent::Announce(_, _)) {
            return event;
        }
    }
}

#[test]
fn real_probe_completion_reports_service_fullname_then_hostname_and_interface() {
    let (daemon, _) = daemon();
    let cfg = config();
    let info = registration(&cfg, &cfg.services[0]).unwrap();
    let event = announce(&daemon, info.clone());
    let DaemonEvent::Announce(fullname, address) = &event else { unreachable!() };
    assert_eq!(fullname, info.get_fullname());
    let (host, interface) = address.split_once(':').unwrap();
    assert_eq!(host, info.get_hostname());
    assert!(!interface.is_empty());
    let mut discovery =
        Discovery { name: "previous".into(), joined: ["192.0.2.1".parse().unwrap()].into(), announced: false };
    discovery.event(event).unwrap();
    assert_eq!(discovery.status(), Status { state: State::Running, name: cfg.name });
    daemon.shutdown().unwrap().recv_timeout(crate::ui::lifecycle::test_wait::HANG_GUARD).unwrap();
}

fn skip_name(packet: &[u8], offset: &mut usize) {
    loop {
        let len = packet[*offset];
        *offset += 1;
        if len & 0xc0 == 0xc0 {
            *offset += 1;
            return;
        }
        if len == 0 {
            return;
        }
        *offset += usize::from(len);
    }
}

fn txt_data(packet: &[u8]) -> &[u8] {
    let word = |offset| u16::from_be_bytes([packet[offset], packet[offset + 1]]) as usize;
    let mut offset = 12;
    for _ in 0..word(4) {
        skip_name(packet, &mut offset);
        offset += 4;
    }
    for _ in 0..word(6) + word(8) + word(10) {
        skip_name(packet, &mut offset);
        let kind = word(offset);
        let len = word(offset + 8);
        offset += 10;
        if kind == 16 {
            return &packet[offset..offset + len];
        }
        offset += len;
    }
    panic!("daemon response must contain a TXT record");
}

fn query_txt(port: u16, fullname: &str) -> Vec<u8> {
    let socket = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    socket.set_read_timeout(Some(crate::ui::lifecycle::test_wait::HANG_GUARD)).unwrap();
    let mut query = vec![0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0];
    for label in fullname.trim_end_matches('.').split('.') {
        query.push(label.len() as u8);
        query.extend_from_slice(label.as_bytes());
    }
    query.extend_from_slice(&[0, 0, 16, 0, 1]);
    socket.send_to(&query, (Ipv4Addr::LOCALHOST, port)).unwrap();
    let mut packet = [0; 4096];
    let len = socket.recv(&mut packet).unwrap();
    txt_data(&packet[..len]).to_vec()
}

#[test]
fn txt_wire_preserves_empty_records_empty_values_and_key_only_properties() {
    let (daemon, port) = daemon();
    let cfg = config();
    for (txt, expected) in [
        (vec![], vec![0]),
        (
            vec![("k".into(), Some("".into())), ("bare".into(), None), ("a".into(), Some("b=c".into()))],
            b"\x02k=\x04bare\x05a=b=c".to_vec(),
        ),
    ] {
        let service = Service { kind: "_http._tcp".into(), port: 8080, txt };
        let info = registration(&cfg, &service).unwrap();
        let fullname = info.get_fullname().to_string();
        announce(&daemon, info);
        assert_eq!(query_txt(port, &fullname), expected);
        daemon.unregister(&fullname).unwrap().recv_timeout(crate::ui::lifecycle::test_wait::HANG_GUARD).unwrap();
    }
    daemon.shutdown().unwrap().recv_timeout(crate::ui::lifecycle::test_wait::HANG_GUARD).unwrap();
}
