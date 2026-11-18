// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! VXI-11 (VXIbus Consortium VXI-11 TCP/IP Instrument Protocol 1.0): what VISA libraries use for
//! `TCPIP::<ip>::INSTR`. ONC RPC (RFC 5531) with XDR (RFC 4506): a portmapper (program 100000 v2, TCP and UDP port
//! 111) that answers GETPORT for the core channel, and the core channel (program 0x0607AF v1, TCP `CORE_PORT`) with
//! create_link, device_write, device_read, device_readstb, device_clear, remote/local/lock/unlock, destroy_link.
//! The frame thread owns links and SCPI programs; netio owns sockets and timed output.
//! Unauthenticated like SCPI and the web remote: bench LAN only.

pub(crate) use crate::netio::port::rpc::*;

/// One VXI-11 link's output (responses waiting for device_read).
#[derive(Default)]
pub struct Link {
    pub out: Vec<u8>,
    programs: std::collections::VecDeque<crate::scpi::Program>,
    owner: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
}

/// Core channel procedures; `exec` runs one program message (may contain several ';'-separated commands) and
/// returns the response bytes (without the newline).
pub fn core_async(
    c: &Call,
    links: &mut Vec<Option<Link>>,
    owner: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    exec: &mut dyn FnMut(&mut crate::scpi::Program) -> Option<Vec<u8>>,
) -> Vec<u8> {
    if c.prog != CORE_PROG || c.vers != 1 {
        return reply(c.xid, 1);
    }
    let mut x = Xdr::new(c.args);
    let mut o = reply(c.xid, 0);
    match c.proc_ {
        0 => {}
        // create_link(clientId, lockDevice, lock_timeout, device) -> error, lid, abortPort, maxRecvSize
        10 => {
            let lid = links.iter().position(Option::is_none).unwrap_or(links.len());
            if lid >= MAX_LINKS {
                words(&mut o, &[9, 0, 0, 0]); // out of resources
                return o;
            }
            if lid == links.len() {
                links.push(None);
            }
            links[lid] = Some(Link { owner, ..Default::default() });
            words(&mut o, &[0, lid as u32, 0, 1 << 20]);
        }
        // device_write(lid, io_timeout, lock_timeout, flags, data) -> error, size
        11 => {
            let lid = x.u32().unwrap_or(u32::MAX) as usize;
            let _ = (x.u32(), x.u32(), x.u32());
            let data = x.opaque().unwrap_or(&[]);
            match links.get_mut(lid).and_then(|l| l.as_mut()) {
                Some(l) => {
                    let queued: usize = l.programs.iter().map(|program| program.bytes).sum();
                    if queued + data.len() > MAX_RECORD || l.programs.len() >= 16 {
                        words(&mut o, &[9, 0]);
                        return o;
                    }
                    l.programs.push_back(crate::scpi::Program::new(&String::from_utf8_lossy(data)));
                    pump_link(l, exec);
                    words(&mut o, &[0, data.len() as u32]);
                }
                None => {
                    words(&mut o, &[4, 0]); // invalid link identifier
                }
            }
        }
        // device_read(lid, requestSize, io_timeout, lock_timeout, flags, termChar) -> error, reason, data
        12 => {
            let lid = x.u32().unwrap_or(u32::MAX) as usize;
            let req = x.u32().unwrap_or(0) as usize;
            match links.get_mut(lid).and_then(|l| l.as_mut()) {
                Some(l) if !l.out.is_empty() => {
                    let n = req.max(1).min(l.out.len());
                    let chunk: Vec<u8> = l.out.drain(..n).collect();
                    words(&mut o, &[0, if l.out.is_empty() { 4 } else { 1 }]); // END, or REQCNT
                    put_opaque(&mut o, &chunk);
                }
                link => {
                    words(&mut o, &[if link.is_some() { 15 } else { 4 }, 0]); // timeout or invalid link
                    put_opaque(&mut o, &[]);
                }
            }
        }
        // device_readstb -> error, stb
        13 => {
            words(&mut o, &[0, 0]);
        }
        // device_trigger, device_clear, device_remote, device_local, device_lock, device_unlock -> error
        14..=19 => {
            if c.proc_ == 15 {
                let lid = x.u32().unwrap_or(u32::MAX) as usize;
                if let Some(Some(l)) = links.get_mut(lid) {
                    l.out.clear();
                    l.programs.clear();
                }
            }
            put(&mut o, 0);
        }
        // destroy_link(lid) -> error
        23 => {
            let lid = x.u32().unwrap_or(u32::MAX) as usize;
            if let Some(l) = links.get_mut(lid) {
                *l = None;
            }
            put(&mut o, 0);
        }
        _ => return reply(c.xid, 3),
    }
    o
}

fn pump_link(link: &mut Link, exec: &mut dyn FnMut(&mut crate::scpi::Program) -> Option<Vec<u8>>) {
    while let Some(mut program) = link.programs.pop_front() {
        if link.out.len() >= crate::scpi::RESP_MAX {
            link.programs.clear();
            break;
        }
        let Some(response) = exec(&mut program) else {
            link.programs.push_front(program);
            break;
        };
        if program.has_reply {
            link.out.extend_from_slice(&response);
            link.out.push(b'\n');
        }
    }
}

#[cfg(test)]
pub fn core(c: &Call, links: &mut Vec<Option<Link>>, exec: &mut dyn FnMut(&str) -> Vec<u8>) -> Vec<u8> {
    core_async(c, links, None, &mut |program| Some(program.immediate(exec)))
}

const MAX_LINKS: usize = 16;

pub use ports::Server;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_vxi_listeners_use_loopback_and_advertise_the_selected_core() {
        use std::{
            io::{Read, Write},
            sync::Arc,
        };
        let mut s =
            Server::host(crate::netio::Runtime::new().unwrap(), Arc::new(crate::netio::port::Wake::new().unwrap()))
                .unwrap();
        s.poll_async(&mut |_| panic!("portmapper does not interpret SCPI"));
        let (tcp, udp, core) = s.ports().unwrap();
        let client = std::thread::spawn(move || {
            let mut args = Vec::new();
            for v in [CORE_PROG, 1, 6, 0] {
                put(&mut args, v);
            }
            let msg = call(1, PMAP_PROG, 3, &args);
            let socket = std::net::UdpSocket::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
            socket.set_read_timeout(Some(crate::ui::lifecycle::test_wait::HANG_GUARD)).unwrap();
            socket.send_to(&msg, (std::net::Ipv4Addr::LOCALHOST, udp)).unwrap();
            let mut response = [0; 1024];
            let n = socket.recv(&mut response).unwrap();
            assert_eq!(Xdr::new(&response[24..n]).u32(), Some(u32::from(core)));
            let mut socket = std::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, tcp)).unwrap();
            socket.set_read_timeout(Some(crate::ui::lifecycle::test_wait::HANG_GUARD)).unwrap();
            socket.write_all(&((msg.len() as u32) | 1 << 31).to_be_bytes()).unwrap();
            socket.write_all(&msg).unwrap();
            let mut header = [0; 4];
            socket.read_exact(&mut header).unwrap();
            let n = (u32::from_be_bytes(header) & 0x7fffffff) as usize;
            socket.read_exact(&mut response[..n]).unwrap();
            assert_eq!(Xdr::new(&response[24..n]).u32(), Some(u32::from(core)));
        });
        let guard = crate::ui::lifecycle::test_wait::Wait::new("portmapper IO completion");
        while !client.is_finished() {
            guard.check();
            s.poll_async(&mut |_| panic!("portmapper does not interpret SCPI"));
            std::thread::yield_now();
        }
        client.join().unwrap();
    }

    #[test]
    fn getport_uses_the_supplied_port_instead_of_the_device_default() {
        let mut args = Vec::new();
        for v in [CORE_PROG, 1, 6, 0] {
            put(&mut args, v);
        }
        let msg = call(1, PMAP_PROG, 3, &args);
        let reply = portmap(&parse_call(&msg).unwrap(), 43210);
        assert_eq!(Xdr::new(&reply[24..]).u32(), Some(43210));
    }

    fn call(xid: u32, prog: u32, proc_: u32, args: &[u8]) -> Vec<u8> {
        let mut m = Vec::new();
        for v in [xid, 0, 2, prog, 1, proc_, 0, 0, 0, 0] {
            put(&mut m, v);
        }
        m.extend_from_slice(args);
        m
    }

    #[test]
    fn empty_query_has_a_readable_end_marker_and_cancelled_fence_has_no_reply() {
        let mut ui = crate::ui::Ui::for_tests(crate::wave::Source::Demo { t: 0.0 });
        let cv = crate::gfx::Canvas::new(crate::ui::W, crate::ui::H);
        let mut scpi = crate::scpi::Scpi::open(0, false);
        let mut links = vec![Some(Link::default())];
        let link = links[0].as_mut().unwrap();
        link.programs.push_back(crate::scpi::Program::new(":MEAS:CLEAR;:MEAS:STRIP?"));
        pump_link(link, &mut |p| scpi.poll_program(&mut ui, &cv, p));
        assert_eq!(link.out, b"\n");
        let mut args = Vec::new();
        for value in [0, 1024, 1000, 0, 0, 0] {
            put(&mut args, value);
        }
        let request = call(77, CORE_PROG, 12, &args);
        let reply =
            core_async(&parse_call(&request).unwrap(), &mut links, None, &mut |p| scpi.poll_program(&mut ui, &cv, p));
        let mut body = Xdr::new(&reply[24..]);
        assert_eq!((body.u32(), body.u32()), (Some(0), Some(4)));
        assert_eq!(body.opaque(), Some(&b"\n"[..]), "empty query succeeds instead of an immediate read timeout");
        let link = links[0].as_mut().unwrap();
        link.programs.push_back(crate::scpi::Program::new(":MEAS:CLEAR"));
        pump_link(link, &mut |p| scpi.poll_program(&mut ui, &cv, p));
        assert!(link.out.is_empty());
        link.programs.push_back(crate::scpi::Program::new(":MEAS:STRIP?;*OPC?"));
        pump_link(link, &mut |p| scpi.poll_program(&mut ui, &cv, p));
        assert_eq!(std::mem::take(&mut link.out), b";1\n");
        link.programs.push_back(crate::scpi::Program::new(":MEAS:STRIP?;:TRIG:EDGE:LEV 10;:SINGLE;*OPC?;:RUN"));
        pump_link(link, &mut |p| scpi.poll_program(&mut ui, &cv, p));
        assert!(link.out.is_empty());
        assert_eq!(link.programs.len(), 1);
        scpi.exec_line(&mut ui, &cv, ":STOP");
        pump_link(link, &mut |p| scpi.poll_program(&mut ui, &cv, p));
        assert!(link.out.is_empty());
        assert!(link.programs.is_empty());
        assert_eq!(ui.m.run, crate::model::RunState::Stop);
    }

    #[test]
    fn getport_and_a_query_round_trip() {
        // portmapper GETPORT(core, v1, TCP)
        let mut a = Vec::new();
        for v in [CORE_PROG, 1, 6, 0] {
            put(&mut a, v);
        }
        let m = call(7, PMAP_PROG, 3, &a);
        let r = portmap(&parse_call(&m).unwrap(), CORE_PORT);
        let mut x = Xdr::new(&r);
        assert_eq!(
            [x.u32(), x.u32(), x.u32(), x.u32(), x.u32(), x.u32(), x.u32()],
            [Some(7), Some(1), Some(0), Some(0), Some(0), Some(0), Some(CORE_PORT as u32)]
        );
        // create_link, device_write "*IDN?\n", device_read, destroy_link
        let mut links = Vec::new();
        let mut exec = |c: &str| if c == "*IDN?" { b"OOS,DHO924S".to_vec() } else { Vec::new() };
        let mut a = Vec::new();
        for v in [1, 0, 0] {
            put(&mut a, v);
        }
        put_opaque(&mut a, b"inst0");
        let r = core(&parse_call(&call(8, CORE_PROG, 10, &a)).unwrap(), &mut links, &mut exec);
        let mut x = Xdr::new(&r[24..]);
        assert_eq!((x.u32(), x.u32()), (Some(0), Some(0)));
        let mut a = Vec::new();
        for v in [0, 1000, 0, 8] {
            put(&mut a, v);
        }
        put_opaque(&mut a, b"*IDN?\n");
        let r = core(&parse_call(&call(9, CORE_PROG, 11, &a)).unwrap(), &mut links, &mut exec);
        let mut x = Xdr::new(&r[24..]);
        assert_eq!((x.u32(), x.u32()), (Some(0), Some(6)));
        let mut a = Vec::new();
        for v in [0, 1024, 1000, 0, 0, 0] {
            put(&mut a, v);
        }
        let r = core(&parse_call(&call(10, CORE_PROG, 12, &a)).unwrap(), &mut links, &mut exec);
        let mut x = Xdr::new(&r[24..]);
        assert_eq!((x.u32(), x.u32()), (Some(0), Some(4)));
        assert_eq!(x.opaque(), Some(&b"OOS,DHO924S\n"[..]));
        let mut a = Vec::new();
        put(&mut a, 0);
        core(&parse_call(&call(11, CORE_PROG, 23, &a)).unwrap(), &mut links, &mut exec);
        assert!(links[0].is_none());
    }

    #[test]
    fn verifier_without_its_padding_is_refused() {
        let mut m = Vec::new();
        for v in [1, 0, 2, CORE_PROG, 1, 0, 0, 0, 0, 1] {
            put(&mut m, v);
        }
        m.push(0xff); // verifier of length 1, the 3 pad bytes are missing
        assert!(parse_call(&m).is_none());
    }

    #[test]
    fn record_marking_joins_fragments() {
        let mut b = Vec::new();
        b.extend_from_slice(&3u32.to_be_bytes());
        b.extend_from_slice(b"abc");
        b.extend_from_slice(&(2u32 | 1 << 31).to_be_bytes());
        b.extend_from_slice(b"de");
        b.extend_from_slice(&[0, 0]); // start of the next record
        assert_eq!(take_record(&mut b), Some(b"abcde".to_vec()));
        assert_eq!(b, vec![0, 0]);
        assert_eq!(take_record(&mut b), None);
    }
}

#[cfg(test)]
mod empty_tests;
mod ports;
