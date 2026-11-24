// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;
use std::io::Write;

#[test]
fn fragmented_pipelined_programs_and_two_clients_share_interpreter_state() {
    let mut scpi = Scpi::open(crate::net6::Port::Host, false);
    let mut ui = Ui::for_tests(crate::wave::Source::Demo { t: 0.0 });
    let cv = Canvas::new(crate::ui::W, crate::ui::H);
    let mut first = TcpStream::connect(("127.0.0.1", scpi.port().unwrap())).unwrap();
    let mut second = TcpStream::connect(("127.0.0.1", scpi.port().unwrap())).unwrap();
    first.write_all(b":WAV:SO").unwrap();
    first.write_all(b"UR CHAN2;:WAV:FORM WORD;:WAV:SOUR?;:WAV:FORM?\n*OPC?\n").unwrap();
    let bytes = test_io::bytes(&mut first, || scpi.poll(&mut ui, &cv), b"CHAN2;WORD\n1\n".len());
    assert_eq!(bytes, b"CHAN2;WORD\n1\n");
    second.write_all(b":WAV:SOUR?;:WAV:FORM?;:BAD?\n").unwrap();
    let answer = test_io::line(&mut second, || scpi.poll(&mut ui, &cv));
    assert!(answer.starts_with("CHAN2;WORD;-113,"), "{answer}");
    first.write_all(b":SYST:ERR?;*ESR?\n").unwrap();
    let answer = test_io::line(&mut first, || scpi.poll(&mut ui, &cv));
    assert!(answer.starts_with("-113,") && answer.ends_with(";32\n"), "{answer}");
    second.write_all(b":SYST:ERR?;*ESR?\n").unwrap();
    assert_eq!(test_io::line(&mut second, || scpi.poll(&mut ui, &cv)), "0,\"No error\";0\n");
}

#[test]
fn waveform_source_and_format_are_atomic_across_a_continuation() {
    let (mut scpi, mut ui, cv) = super::tests::reference_session();
    let expected = scpi.exec_line(&mut ui, &cv, ":WAV:SOUR R1;:WAV:FORM WORD;:WAV:DATA?");
    scpi.exec_line(&mut ui, &cv, ":TRIG:EDGE:LEV 10");
    let mut first = TcpStream::connect(("127.0.0.1", scpi.port().unwrap())).unwrap();
    first.write_all(b":WAV:SOUR R1;:WAV:FORM WORD;:SINGLE;*OPC?;:WAV:DATA?\n").unwrap();
    crate::ui::lifecycle::test_wait::until("waveform program continuation", || {
        scpi.poll(&mut ui, &cv);
        scpi.transport.intake.pending().then_some(())
    });
    let mut second = TcpStream::connect(("127.0.0.1", scpi.port().unwrap())).unwrap();
    second.write_all(b":WAV:SOUR R2;:WAV:FORM BYTE;:WAV:DATA?\n").unwrap();
    assert_eq!(test_io::line(&mut second, || scpi.poll(&mut ui, &cv)), "#9000000000\n");
    scpi.exec_line(&mut ui, &cv, ":TRIG:FORC");
    let answer = test_io::bytes(
        &mut first,
        || {
            scpi.poll(&mut ui, &cv);
            ui.step(0.2);
        },
        expected.len() + 3,
    );
    let mut full = b"1;".to_vec();
    full.extend_from_slice(&expected);
    full.push(b'\n');
    assert_eq!(answer, full);
    assert_eq!(scpi.exec_line(&mut ui, &cv, ":WAV:SOUR?;:WAV:FORM?"), b"R2;BYTE");
}

#[test]
fn system_state_exposes_server_side_enqueue_quantiles() {
    let mut scpi = Scpi::open(crate::net6::Port::Host, false);
    let mut ui = Ui::for_tests(crate::wave::Source::Demo { t: 0.0 });
    let cv = Canvas::new(crate::ui::W, crate::ui::H);
    let mut client = TcpStream::connect(("127.0.0.1", scpi.port().unwrap())).unwrap();
    client.write_all(b"*IDN?\n").unwrap();
    test_io::line(&mut client, || scpi.poll(&mut ui, &cv));
    client.write_all(b":SYST:STAT?\n").unwrap();
    let text = test_io::line(&mut client, || scpi.poll(&mut ui, &cv));
    let enqueue = text.split(" diag remote.enqueue ").nth(1).expect("server enqueue diagnostics");
    assert!(enqueue.contains("p50_ns=") && enqueue.contains("p99_ns="), "{text}");
}

#[test]
fn new_raw_client_gets_a_query_reply_during_single_opc() {
    use std::io::{ErrorKind, Read};
    let (mut scpi, mut ui, cv) = super::tests::reference_session();
    scpi.exec_line(&mut ui, &cv, ":TRIG:EDGE:LEV 10");
    let address = ("127.0.0.1", scpi.port().unwrap());
    let mut first = TcpStream::connect(address).unwrap();
    first.set_nonblocking(true).unwrap();
    first.write_all(b":SING;*OPC?\n").unwrap();
    crate::ui::lifecycle::test_wait::until("first client reaches OPC fence", || {
        scpi.poll(&mut ui, &cv);
        scpi.transport.intake.pending().then_some(())
    });
    let ports = scpi.transport.endpoint.ports(&scpi.acl).unwrap();
    let mut second = TcpStream::connect(address).unwrap();
    ui.step(0.02);
    second.write_all(b":TRIG:STAT?\n").unwrap();
    crate::ui::lifecycle::test_wait::until("new connection submits a query", || {
        (!ports.requests().is_empty()).then_some(())
    });
    crate::ui::lifecycle::test_wait::until("accept query behind the first client's fence", || {
        scpi.poll(&mut ui, &cv);
        ports.requests().is_empty().then_some(())
    });
    assert_eq!(test_io::line(&mut second, || scpi.poll(&mut ui, &cv)), "WAIT\n");
    assert_eq!(first.read(&mut [0; 16]).unwrap_err().kind(), ErrorKind::WouldBlock);
    scpi.exec_line(&mut ui, &cv, ":TRIG:FORC");
    assert_eq!(
        test_io::line(&mut first, || {
            ui.step(0.02);
            scpi.poll(&mut ui, &cv);
        }),
        "1\n"
    );
}
