// SPDX-License-Identifier: GPL-2.0-or-later

use std::{
    io::{self, Read, Write},
    net::{SocketAddr, TcpStream, ToSocketAddrs},
    sync::mpsc::{self, RecvTimeoutError},
    thread,
    time::{Duration, Instant},
};

const PORT: u16 = 5555;
const TIMEOUT: Duration = Duration::from_secs(10);
const PACING: Duration = Duration::from_millis(150);
const MAX_REPLY: usize = 65_536;
const STATE: &str = ":SYST:STATE?";

pub(super) trait Scpi {
    fn send(&mut self, command: &str) -> Result<(), String>;
    fn acquiring(&mut self) -> Result<bool, String>;
    fn reconnect(&mut self) -> Result<(), String>;
}

pub(super) struct TcpScpi {
    host: String,
    stream: TcpStream,
    next_command: Instant,
}

impl TcpScpi {
    pub(super) fn connect(host: String) -> Result<Self, String> {
        let deadline = Instant::now() + TIMEOUT;
        let mut error = "SCPI connect: no addresses".to_owned();
        for address in resolve(host.clone(), deadline)? {
            match TcpStream::connect_timeout(&address, remaining(deadline, "SCPI connect")?) {
                Ok(stream) => {
                    stream.set_write_timeout(Some(TIMEOUT)).map_err(|_| "configure SCPI write timeout")?;
                    return Ok(Self { host, stream, next_command: Instant::now() });
                }
                Err(err) => error = diagnostic("SCPI connect", err),
            }
        }
        Err(error)
    }

    fn read_state(&mut self) -> Result<bool, String> {
        let operation = "read SCPI :SYST:STATE? reply";
        let deadline = Instant::now() + TIMEOUT;
        let mut reply = Vec::new();
        let mut chunk = [0; 4096];
        while reply.len() < MAX_REPLY {
            self.stream
                .set_read_timeout(Some(remaining(deadline, operation)?))
                .map_err(|e| diagnostic(operation, e))?;
            let count = chunk.len().min(MAX_REPLY - reply.len());
            let count = self.stream.read(&mut chunk[..count]).map_err(|e| diagnostic(operation, e))?;
            if count == 0 {
                break;
            }
            let bytes = &chunk[..count];
            if let Some(end) = bytes.iter().position(|&b| b == b'\n') {
                reply.extend_from_slice(&bytes[..end]);
                return parse_state(&reply);
            }
            reply.extend_from_slice(bytes);
        }
        Err("SCPI state reply did not contain a complete line".into())
    }
}

impl Scpi for TcpScpi {
    fn send(&mut self, command: &str) -> Result<(), String> {
        thread::sleep(self.next_command.saturating_duration_since(Instant::now()));
        let result = self.stream.write_all(format!("{command}\n").as_bytes());
        self.next_command = Instant::now() + PACING;
        result.map_err(|e| diagnostic(&format!("write SCPI {command}"), e))
    }

    fn acquiring(&mut self) -> Result<bool, String> {
        self.send(STATE)?;
        self.read_state()
    }

    fn reconnect(&mut self) -> Result<(), String> {
        *self = Self::connect(self.host.clone())?;
        Ok(())
    }
}

fn resolve(host: String, deadline: Instant) -> Result<Vec<SocketAddr>, String> {
    let (send, receive) = mpsc::channel();
    // The system resolver may block beyond the connect deadline.
    thread::Builder::new()
        .spawn(move || {
            // The receiver can already have returned its hostname lookup timeout.
            let _ = send.send((host.as_str(), PORT).to_socket_addrs().map(Iterator::collect));
        })
        .map_err(|_| "start SCPI hostname lookup")?;
    match receive.recv_timeout(remaining(deadline, "SCPI hostname lookup")?) {
        Ok(addresses) => addresses.map_err(|e| diagnostic("SCPI hostname lookup", e)),
        Err(RecvTimeoutError::Timeout) => Err("SCPI hostname lookup: timed out".into()),
        Err(RecvTimeoutError::Disconnected) => Err("SCPI hostname lookup: failed".into()),
    }
}

fn remaining(deadline: Instant, operation: &str) -> Result<Duration, String> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Err(format!("{operation}: timed out"));
    }
    Ok(remaining)
}

fn diagnostic(operation: &str, error: io::Error) -> String {
    let detail = match error.kind() {
        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock => "timed out",
        _ => "failed",
    };
    format!("{operation}: {detail}")
}

fn parse_state(reply: &[u8]) -> Result<bool, String> {
    let mut fields = reply.split(u8::is_ascii_whitespace).filter(|field| !field.is_empty());
    if fields.next() == Some(b"fps") && fields.nth(5) == Some(b"run") {
        match fields.next() {
            Some(b"Run") => return Ok(true),
            Some(b"Stop") => return Ok(false),
            _ => {}
        }
    }
    Err("SCPI state reply did not identify acquisition state".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_state_header_and_ignores_trailing_counters() {
        assert!(parse_state(b"fps 30 frames 12 records 345 run Run capture 99\r").unwrap());
        assert!(!parse_state(b"fps 0 frames 12 records 345 run Stop").unwrap());
        assert!(parse_state(b"fps 0 frames 12 records 345 run Single").is_err());
        assert!(parse_state(b"example.invalid run Stop").is_err());
    }

    #[test]
    fn socket_timeout_names_operation_without_io_details() {
        let error = io::Error::new(io::ErrorKind::TimedOut, "192.0.2.1");
        assert_eq!(diagnostic("read SCPI :SYST:STATE? reply", error), "read SCPI :SYST:STATE? reply: timed out");
    }
}
