// SPDX-License-Identifier: GPL-2.0-or-later

use std::{
    io::{self, Read, Write},
    net::{SocketAddr, TcpStream, ToSocketAddrs},
    sync::mpsc::{self, RecvTimeoutError},
    thread,
    time::{Duration, Instant},
};

pub(crate) const PORT: u16 = 5555;
const TIMEOUT: Duration = Duration::from_secs(10);
const PACING: Duration = Duration::from_millis(150);
const MAX_REPLY: usize = 65_536;
pub(crate) const STATE: &str = ":SYST:STATE?";

pub(crate) trait Scpi {
    fn send(&mut self, command: &str) -> Result<(), String>;
    fn query(&mut self, command: &str) -> Result<String, String> {
        self.query_timed(command, || ()).map(|reply| reply.line)
    }
    /// Sample the same clock immediately before the write and once the reply line arrives.
    fn query_timed<T>(&mut self, command: &str, now: impl FnMut() -> T) -> Result<TimedReply<T>, String>;
    fn acquiring(&mut self) -> Result<bool, String> {
        parse_state(&self.query(STATE)?)
    }
    fn reconnect(&mut self) -> Result<(), String>;
}

#[derive(Debug)]
pub(crate) struct TimedReply<T> {
    pub(crate) line: String,
    pub(crate) before_write: T,
    pub(crate) after_line: T,
}

#[derive(Debug)]
pub(crate) struct TcpScpi {
    host: String,
    port: u16,
    stream: Option<TcpStream>,
    next_command: Instant,
    pace: Duration,
}

impl TcpScpi {
    pub(crate) fn connect(host: String, port: u16) -> Result<Self, String> {
        Self::connect_with_pace(host, port, PACING)
    }

    pub(crate) fn connect_with_pace(host: String, port: u16, pace: Duration) -> Result<Self, String> {
        let deadline = Instant::now() + TIMEOUT;
        let mut error = "SCPI connect: no addresses".to_owned();
        for address in resolve(host.clone(), port, deadline)? {
            match TcpStream::connect_timeout(&address, remaining(deadline, "SCPI connect")?) {
                Ok(stream) => {
                    stream.set_write_timeout(Some(TIMEOUT)).map_err(|_| "configure SCPI write timeout")?;
                    return Ok(Self { host, port, stream: Some(stream), next_command: Instant::now(), pace });
                }
                Err(err) => error = diagnostic("SCPI connect", err),
            }
        }
        Err(error)
    }

    fn write_command<T>(&mut self, command: &str, before_write: impl FnOnce() -> T) -> Result<T, String> {
        let command_line = format!("{command}\n");
        let delay = self.next_command.saturating_duration_since(Instant::now());
        if !delay.is_zero() {
            thread::sleep(delay);
        }
        if self.stream.is_none() {
            self.reconnect()?;
        }
        let stream = self.stream.as_mut().ok_or("SCPI connection unavailable")?;
        let stamp = before_write();
        let result = stream.write_all(command_line.as_bytes());
        self.next_command = Instant::now() + self.pace;
        if result.is_err() {
            self.stream = None;
        }
        result.map_err(|e| diagnostic(&operation("write", command), e))?;
        Ok(stamp)
    }
}

fn read_line<T>(
    stream: &mut TcpStream,
    operation: &str,
    after_line: impl FnOnce() -> T,
) -> Result<(String, T), String> {
    let deadline = Instant::now() + TIMEOUT;
    let mut reply = Vec::new();
    let mut chunk = [0; 4096];
    while reply.len() < MAX_REPLY {
        stream.set_read_timeout(Some(remaining(deadline, operation)?)).map_err(|e| diagnostic(operation, e))?;
        let count = chunk.len().min(MAX_REPLY - reply.len());
        let count = stream.read(&mut chunk[..count]).map_err(|e| diagnostic(operation, e))?;
        if count == 0 {
            break;
        }
        let bytes = &chunk[..count];
        if let Some(end) = bytes.iter().position(|&b| b == b'\n') {
            let stamp = after_line();
            reply.extend_from_slice(&bytes[..end]);
            if reply.last() == Some(&b'\r') {
                reply.pop();
            }
            let line = String::from_utf8(reply).map_err(|_| format!("{operation}: invalid UTF-8"))?;
            return Ok((line, stamp));
        }
        reply.extend_from_slice(bytes);
    }
    Err(format!("{operation}: reply did not contain a complete line"))
}

impl Scpi for TcpScpi {
    fn send(&mut self, command: &str) -> Result<(), String> {
        self.write_command(command, || ())
    }

    fn query_timed<T>(&mut self, command: &str, mut now: impl FnMut() -> T) -> Result<TimedReply<T>, String> {
        let before_write = self.write_command(command, &mut now)?;
        let stream = self.stream.as_mut().ok_or("SCPI connection unavailable")?;
        let result = read_line(stream, &operation("read", command), now);
        if result.is_err() {
            // A late reply belongs to this request. Discard the stream before the next command.
            self.stream = None;
        }
        let (line, after_line) = result?;
        Ok(TimedReply { line, before_write, after_line })
    }

    fn reconnect(&mut self) -> Result<(), String> {
        *self = Self::connect_with_pace(self.host.clone(), self.port, self.pace)?;
        Ok(())
    }
}

fn operation(action: &str, command: &str) -> String {
    let header = command.split([' ', ';']).next().unwrap_or_default();
    format!("{action} SCPI {header}")
}

fn resolve(host: String, port: u16, deadline: Instant) -> Result<Vec<SocketAddr>, String> {
    let (send, receive) = mpsc::channel();
    // The system resolver may block beyond the connect deadline.
    thread::Builder::new()
        .spawn(move || {
            // The receiver can already have returned its hostname lookup timeout.
            let _ = send.send((host.as_str(), port).to_socket_addrs().map(Iterator::collect));
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

pub(crate) fn parse_state(reply: &str) -> Result<bool, String> {
    let mut fields = reply.split_ascii_whitespace();
    if fields.next() == Some("fps") && fields.nth(5) == Some("run") {
        match fields.next() {
            Some("Run") => return Ok(true),
            Some("Stop") => return Ok(false),
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
        assert!(parse_state("fps 30 frames 12 records 345 run Run capture 99\r").unwrap());
        assert!(!parse_state("fps 0 frames 12 records 345 run Stop").unwrap());
        assert!(parse_state("fps 0 frames 12 records 345 run Single").is_err());
        assert!(parse_state("example.invalid run Stop").is_err());
    }

    #[test]
    fn socket_timeout_names_operation_without_io_details() {
        let error = io::Error::new(io::ErrorKind::TimedOut, "192.0.2.1");
        assert_eq!(diagnostic("read SCPI :SYST:STATE? reply", error), "read SCPI :SYST:STATE? reply: timed out");
    }
}

#[cfg(test)]
pub(crate) mod fake;
