// SPDX-License-Identifier: GPL-2.0-or-later
//! Two `oos-ui --serve` hosts side by side: each binds its own loopback ports and storage, and SCPI raw, MCP over
//! HTTP and the VXI-11 port mapper answer on each.

pub mod support;

use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{TcpStream, UdpSocket};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;
use support::{isolated_inputs, Inputs};

/// The identity a host's readiness record must echo.
const BUILD: &str = "0123abcd";
/// Only a hang guard (docs/development.md): startup or a socket that stays silent this long fails the test.
const HANG_GUARD: Duration = Duration::from_secs(120);

/// A running host and the ports from its readiness record. Dropping it kills and reaps the child.
struct Host {
    child: Child,
    ports: Ports,
    inputs: Inputs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Ports {
    web: u16,
    scpi: u16,
    vxi_tcp: u16,
    vxi_udp: u16,
    vxi_core: u16,
}

impl Host {
    fn start() -> Host {
        let inputs = isolated_inputs();
        let stderr = std::fs::File::create(inputs.path("stderr.log")).expect("isolated stderr log must be writable");
        let mut child = Command::new(env!("CARGO_BIN_EXE_oos-ui"))
            .arg("--serve")
            .arg("--store-dir")
            .arg(inputs.path("store"))
            .env("OOS_DEMO_LOOP", "3")
            .env("OOS_HARNESS_BUILD", BUILD)
            .env_remove("OOS_HARNESS_READY")
            .stdout(Stdio::piped())
            .stderr(stderr)
            .spawn()
            .expect("Cargo supplies an executable host binary");
        let record = ready_record(&mut child, |ready| ready.recv_timeout(HANG_GUARD).map_err(io::Error::other)?)
            .unwrap_or_else(|error| {
                let log = std::fs::read_to_string(inputs.path("stderr.log")).unwrap_or_default();
                panic!("host failed before readiness: {error}; stderr:\n{log}");
            });
        assert_eq!(field(&record, "pid"), child.id(), "{record}");
        assert!(record.contains(&format!("\"build\":\"{BUILD}\"")), "{record}");
        let port = |name| u16::try_from(field(&record, name)).expect("a port fits in u16");
        let ports = Ports {
            web: port("web"),
            scpi: port("scpi"),
            vxi_tcp: port("vxi_tcp"),
            vxi_udp: port("vxi_udp"),
            vxi_core: port("vxi_core"),
        };
        Host { child, ports, inputs }
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        // The child may have exited already; the wait reaps it either way.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// The readiness line (`net6::host_ready`), printed once every listener is bound.
fn ready_record(
    child: &mut Child,
    wait: impl FnOnce(Receiver<io::Result<String>>) -> io::Result<String>,
) -> io::Result<String> {
    let mut stdout = BufReader::new(child.stdout.take().expect("stdout is piped"));
    let (send, ready) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = send.send(read_record(&mut stdout));
        // The child may print more; drain it so a full pipe never blocks the host.
        let _ = io::copy(&mut stdout, &mut io::sink());
    });
    let record = wait(ready);
    if record.is_err() {
        let _ = child.kill();
        child.wait().expect("failed startup child must be reaped");
    }
    record
}

fn read_record(stdout: &mut impl BufRead) -> io::Result<String> {
    let mut line = String::new();
    loop {
        line.clear();
        if stdout.read_line(&mut line)? == 0 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "host exited before readiness"));
        }
        if line.starts_with("{\"pid\":") {
            return Ok(line.trim().to_string());
        }
    }
}

#[test]
fn startup_timeout_kills_and_reaps_the_child() {
    let mut child = Command::new("sh")
        .args(["-c", "read -r line"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("sh starts");
    assert!(child.try_wait().expect("child state is readable").is_none());
    let error =
        ready_record(&mut child, |_ready| Err(io::Error::new(io::ErrorKind::TimedOut, "injected startup deadline")))
            .expect_err("a silent child must fail startup on the deadline event");
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    assert!(!child.try_wait().expect("child state is readable").expect("child was reaped").success());
}

#[test]
fn readiness_skips_other_output_and_reports_eof() {
    let record = "{\"pid\":42}\n";
    let output = format!("starting\n{record}more output\n");
    let mut stdout = io::Cursor::new(output);
    assert_eq!(read_record(&mut stdout).unwrap(), record.trim());
    assert_eq!(read_record(&mut stdout).unwrap_err().kind(), io::ErrorKind::UnexpectedEof);
}

/// The unsigned number after `"name":` in a flat JSON record.
fn field(record: &str, name: &str) -> u32 {
    let key = format!("\"{name}\":");
    let at = record.find(&key).map(|i| i + key.len()).unwrap_or_else(|| panic!("{name} missing in {record}"));
    let digits: String = record[at..].chars().take_while(char::is_ascii_digit).collect();
    digits.parse().unwrap_or_else(|_| panic!("{name} is no number in {record}"))
}

fn connect(port: u16) -> TcpStream {
    let stream = TcpStream::connect(("127.0.0.1", port)).expect("loopback listener accepts");
    stream.set_read_timeout(Some(HANG_GUARD)).expect("read timeout");
    stream
}

fn idn(port: u16) -> String {
    let mut stream = connect(port);
    stream.write_all(b"*IDN?\n").expect("SCPI query is sent");
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line).expect("SCPI answers");
    line
}

fn mcp_tools(port: u16) -> String {
    let body = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#;
    let request = format!(
        "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\
         Connection: close\r\n\r\n{body}",
        body.len()
    );
    let mut stream = connect(port);
    stream.write_all(request.as_bytes()).expect("HTTP request is sent");
    let mut answer = String::new();
    stream.read_to_string(&mut answer).expect("HTTP answers and closes");
    answer
}

/// The port the VXI-11 port mapper names for the core channel: an ONC RPC GETPORT call (program 0x0607AF,
/// version 1, TCP).
fn core_port(port: u16) -> u32 {
    let call = [7u32, 0, 2, 100_000, 2, 3, 0, 0, 0, 0, 0x0607AF, 1, 6, 0];
    let bytes: Vec<u8> = call.iter().flat_map(|w| w.to_be_bytes()).collect();
    let socket = UdpSocket::bind(("127.0.0.1", 0)).expect("loopback UDP socket");
    socket.set_read_timeout(Some(HANG_GUARD)).expect("read timeout");
    socket.send_to(&bytes, ("127.0.0.1", port)).expect("GETPORT is sent");
    let mut reply = [0u8; 1024];
    let n = socket.recv(&mut reply).expect("the port mapper answers");
    let last: [u8; 4] = reply[..n][n - 4..].try_into().expect("four bytes");
    u32::from_be_bytes(last)
}

#[test]
fn two_hosts_use_their_own_ports_and_storage_and_answer_on_each() {
    let hosts = [Host::start(), Host::start()];
    assert_ne!(hosts[0].child.id(), hosts[1].child.id());
    assert_ne!(hosts[0].inputs.path("store"), hosts[1].inputs.path("store"));
    let ports = hosts.each_ref().map(|h| {
        let p = h.ports;
        [p.web, p.scpi, p.vxi_tcp, p.vxi_core]
    });
    assert!(ports[0].iter().all(|p| !ports[1].contains(p)), "{ports:?}");
    for host in &hosts {
        let p = host.ports;
        let id = idn(p.scpi);
        assert!(id.to_uppercase().contains("OOS"), "{id}");
        let answer = mcp_tools(p.web);
        assert!(answer.starts_with("HTTP/1.1 200") && answer.contains("\"result\""), "{answer}");
        assert_eq!(core_port(p.vxi_udp), u32::from(p.vxi_core));
    }
}
