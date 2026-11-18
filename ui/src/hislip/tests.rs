// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;
use std::io::{ErrorKind, Read, Write};
use std::net::TcpStream;

/// What the server sends for `out`, in order.
fn sent(out: &Out) -> Vec<Vec<u8>> {
    out.output
        .chunks()
        .flat_map(|bytes| {
            if let Some((msgid, max)) = out.output.hislip_framing() {
                let count = bytes.len().div_ceil(max).max(1);
                (0..count)
                    .map(|k| {
                        encode(
                            if k + 1 == count { DATA_END } else { DATA },
                            0,
                            msgid,
                            &bytes[k * max..((k + 1) * max).min(bytes.len())],
                        )
                    })
                    .collect()
            } else {
                vec![bytes.to_vec()]
            }
        })
        .collect()
}

/// AsyncMaximumMessageSize of 17 bytes (a payload of one byte) made a message per byte of the reply: a 1 MiB reply
/// became a list of a million messages. The payload of a message is now at least MIN_PAYLOAD, and the messages
/// are made as they are sent.
#[test]
fn a_tiny_maximum_message_size_still_gives_messages_of_min_payload() {
    let mut peer = Peer { role: Role::New, session: 0, pending: Vec::new(), msgid: 0, program: None };
    let (mut live, mut next) = (Vec::new(), 1u16);
    let mut exec = |_: &str| vec![b'a'; 1 << 20];
    let init = Message { typ: INITIALIZE, control: 0, param: 0x0100_0000, data: b"hislip0".to_vec() };
    on_message(&mut peer, &mut live, &mut next, &init, &mut exec);
    let size = Message { typ: ASYNC_MAXIMUM_MESSAGE_SIZE, control: 0, param: 0, data: 17u64.to_be_bytes().into() };
    on_message(&mut peer, &mut live, &mut next, &size, &mut exec);
    let query = Message { typ: DATA_END, control: 0, param: 5, data: b"X?\n".to_vec() };
    let out = sent(&on_message(&mut peer, &mut live, &mut next, &query, &mut exec));
    assert_eq!(out.len(), ((1 << 20) + 1usize).div_ceil(MIN_PAYLOAD));
    let mut back = Vec::new();
    for (k, m) in out.iter().enumerate() {
        let m = take_message(&mut m.clone()).unwrap().unwrap();
        assert_eq!((m.typ, m.param), (if k + 1 == out.len() { DATA_END } else { DATA }, 5));
        assert!(m.data.len() <= MIN_PAYLOAD);
        back.extend_from_slice(&m.data);
    }
    assert_eq!((back.len(), back[0], back[back.len() - 1]), ((1 << 20) + 1, b'a', b'\n'));
    // a reply of exactly one message's payload is one DATA_END
    let reply = Out { output: Output::hislip(vec![0; MIN_PAYLOAD], 1, MIN_PAYLOAD), close: false };
    assert_eq!(sent(&reply).len(), 1);
    let empty = Out { output: Output::hislip(Vec::new(), 1, MIN_PAYLOAD), close: false };
    assert_eq!(take_message(&mut sent(&empty)[0]).unwrap().unwrap().typ, DATA_END);
}

fn exec_idn(c: &str) -> Vec<u8> {
    if c == "*IDN?" {
        b"OOS,unit".to_vec()
    } else {
        Vec::new()
    }
}

#[test]
fn empty_query_emits_data_end_but_writes_and_cancelled_fences_do_not() {
    let mut ui = crate::ui::Ui::for_tests(crate::wave::Source::Demo { t: 0.0 });
    let cv = crate::gfx::Canvas::new(crate::ui::W, crate::ui::H);
    let mut scpi = crate::scpi::Scpi::open(0, false);
    let mut peer = Peer { role: Role::Sync, session: 1, pending: Vec::new(), msgid: 0, program: None };
    let mut live = vec![Live { id: 1, max_payload: MAX_PAYLOAD }];
    let mut next = 2;
    let message =
        |id, command: &str| Message { typ: DATA_END, control: 0, param: id, data: command.as_bytes().to_vec() };
    let empty =
        on_program_message(&mut peer, &mut live, &mut next, &message(7, ":MEAS:CLEAR;:MEAS:STRIP?"), &mut |p| {
            scpi.poll_program(&mut ui, &cv, p)
        });
    assert_eq!(sent(&empty).len(), 1);
    let reply = take_message(&mut sent(&empty)[0]).unwrap().unwrap();
    assert_eq!((reply.typ, reply.param, reply.data), (DATA_END, 7, vec![b'\n']));
    let write = on_program_message(&mut peer, &mut live, &mut next, &message(8, ":MEAS:CLEAR"), &mut |p| {
        scpi.poll_program(&mut ui, &cv, p)
    });
    assert!(sent(&write).is_empty());
    let compound = on_program_message(&mut peer, &mut live, &mut next, &message(9, ":MEAS:STRIP?;*OPC?"), &mut |p| {
        scpi.poll_program(&mut ui, &cv, p)
    });
    assert_eq!(take_message(&mut sent(&compound)[0]).unwrap().unwrap().data, b";1\n");
    let pending = on_program_message(
        &mut peer,
        &mut live,
        &mut next,
        &message(10, ":MEAS:STRIP?;:TRIG:EDGE:LEV 10;:SINGLE;*OPC?;:RUN"),
        &mut |p| scpi.poll_program(&mut ui, &cv, p),
    );
    assert!(sent(&pending).is_empty(), "an earlier empty query must not emit a premature response at a pending fence");
    let mut program = peer.program.take().unwrap();
    assert!(program.has_reply);
    scpi.exec_line(&mut ui, &cv, ":STOP");
    assert_eq!(scpi.poll_program(&mut ui, &cv, &mut program), Some(Vec::new()));
    assert!(!program.has_reply, "cancelled queries must not emit an empty DATA_END response");
    assert_eq!(ui.m.run, crate::model::RunState::Stop);
}

#[test]
fn the_payload_is_the_scpi_and_the_lock_is_not_kept() {
    let mut peer = Peer { role: Role::New, session: 0, pending: Vec::new(), msgid: 0, program: None };
    let mut live = Vec::new();
    let mut next = 1u16;
    let mut exec = exec_idn;
    let init = on_message(
        &mut peer,
        &mut live,
        &mut next,
        &Message { typ: INITIALIZE, control: 0, param: 0x0100_0000, data: b"hislip0".to_vec() },
        &mut exec,
    );
    assert!(!init.close);
    let resp = take_message(&mut sent(&init)[0].clone()).unwrap().unwrap();
    assert_eq!(resp.typ, INITIALIZE_RESPONSE);
    assert_eq!(resp.param >> 16, VERSION as u32);
    let id = resp.param as u16;
    assert_ne!(id, 0);
    // a second client is also told the lock succeeded: no session lock is stored
    let mut other = Peer { role: Role::Sync, session: id, pending: Vec::new(), msgid: 0, program: None };
    let lock = on_message(
        &mut other,
        &mut live,
        &mut next,
        &Message { typ: ASYNC_LOCK, control: 1, param: 0, data: Vec::new() },
        &mut exec,
    );
    let again = on_message(
        &mut peer,
        &mut live,
        &mut next,
        &Message { typ: ASYNC_LOCK, control: 1, param: 0, data: Vec::new() },
        &mut exec,
    );
    assert_eq!(take_message(&mut sent(&lock)[0].clone()).unwrap().unwrap().control, 1);
    assert_eq!(take_message(&mut sent(&again)[0].clone()).unwrap().unwrap().control, 1);
    // the payload may arrive in pieces; only the SCPI text is interpreted
    on_message(
        &mut peer,
        &mut live,
        &mut next,
        &Message { typ: DATA, control: 0, param: 1, data: b"*ID".to_vec() },
        &mut exec,
    );
    let end = on_message(
        &mut peer,
        &mut live,
        &mut next,
        &Message { typ: DATA_END, control: 1, param: 3, data: b"N?\n".to_vec() },
        &mut exec,
    );
    let data = take_message(&mut sent(&end)[0]).unwrap().unwrap();
    assert_eq!(data.typ, DATA_END);
    assert_eq!(data.param, 3);
    assert_eq!(data.data, b"OOS,unit\n");
}

#[test]
fn a_socket_round_trip_answers_idn() {
    let mut h = Server::open(crate::net6::Port::Host);
    let port = h.port().expect("port");
    // the deadlines only guard against a hang: a slow or busy computer must not fail the test
    let t = std::thread::spawn(move || {
        let mut c = TcpStream::connect(("127.0.0.1", port)).unwrap();
        c.set_read_timeout(Some(crate::ui::lifecycle::test_wait::HANG_GUARD)).unwrap();
        c.write_all(&encode(INITIALIZE, 0, 0x0200_4F53, b"hislip0")).unwrap();
        let mut buf = Vec::new();
        let resp = read_one(&mut c, &mut buf);
        assert_eq!(resp.typ, INITIALIZE_RESPONSE);
        assert_eq!(resp.param >> 16, VERSION as u32, "a newer client is answered at 1.0");
        let id = resp.param as u16;
        let mut a = TcpStream::connect(("127.0.0.1", port)).unwrap();
        a.set_read_timeout(Some(crate::ui::lifecycle::test_wait::HANG_GUARD)).unwrap();
        a.write_all(&encode(ASYNC_INITIALIZE, 0, id as u32, b"")).unwrap();
        let mut abuf = Vec::new();
        assert_eq!(read_one(&mut a, &mut abuf).typ, ASYNC_INITIALIZE_RESPONSE);
        c.write_all(&encode(DATA_END, 0, 0xffff_ff00, b"*IDN?\n")).unwrap();
        let data = read_one(&mut c, &mut buf);
        assert_eq!((data.typ, data.data.as_slice()), (DATA_END, &b"OOS,unit\n"[..]));
    });
    let start = crate::ui::lifecycle::test_wait::Wait::new("HiSLIP IDN client completion");
    while !t.is_finished() {
        start.check();
        h.poll(&mut exec_idn);
        start.poll();
    }
    t.join().unwrap();
}

fn read_one(c: &mut TcpStream, buf: &mut Vec<u8>) -> Message {
    let mut tmp = [0u8; 256];
    let start = crate::ui::lifecycle::test_wait::Wait::new("complete HiSLIP message");
    loop {
        if let Ok(Some(m)) = take_message(buf) {
            return m;
        }
        match c.read(&mut tmp) {
            Ok(0) => panic!("closed"),
            Ok(n) => buf.extend_from_slice(&tmp[..n]),
            Err(e) if e.kind() == ErrorKind::WouldBlock || e.kind() == ErrorKind::TimedOut => {
                start.poll();
            }
            Err(e) => panic!("{e}"),
        }
    }
}

#[test]
fn malformed_and_oversized_headers_send_fatal_then_close() {
    for mut header in [encode(DEVICE_CLEAR_COMPLETE, 0, 0, b""), encode(DATA_END, 0, 0, b"")] {
        if header[2] == DEVICE_CLEAR_COMPLETE {
            header[..2].copy_from_slice(b"XX");
        } else {
            header[8..].copy_from_slice(&u64::MAX.to_be_bytes());
        }
        let mut server = Server::open(crate::net6::Port::Host);
        let port = server.port().unwrap();
        let client = std::thread::spawn(move || {
            let mut socket = TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, port)).unwrap();
            socket.set_read_timeout(Some(crate::ui::lifecycle::test_wait::HANG_GUARD)).unwrap();
            socket.write_all(&header).unwrap();
            let fatal = read_one(&mut socket, &mut Vec::new());
            assert_eq!((fatal.typ, fatal.control), (FATAL_ERROR, FATAL_BAD_HEADER));
            assert_eq!(socket.read(&mut [0]).unwrap(), 0);
        });
        let wait = crate::ui::lifecycle::test_wait::Wait::new("HiSLIP fatal response");
        while !client.is_finished() {
            wait.check();
            server.poll(&mut |_| panic!("invalid framing is not SCPI"));
            wait.poll();
        }
        client.join().unwrap();
    }
}
