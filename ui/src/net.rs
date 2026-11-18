// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! Helpers for the remaining synchronous HTTP and HiSLIP adapters.

use std::io::{self, ErrorKind, Write};
use std::net::TcpStream;
use std::time::Duration;

/// Whether a read error only means "nothing now" (try again in the next pass).
pub fn later(e: &io::Error) -> bool {
    matches!(e.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut | io::ErrorKind::Interrupted)
}

/// Send limits for the blocking HiSLIP adapter until its M15b cutover.
const SEND_STALL: Duration = Duration::from_secs(2);
const SEND_LIMIT: Duration = Duration::from_secs(30);

/// Writes `d` to a client; false when it was given up (the connection is then to be closed).
pub fn send(s: &mut TcpStream, d: &[u8]) -> bool {
    let end = std::time::Instant::now() + SEND_LIMIT;
    let _ = s.set_write_timeout(Some(SEND_STALL));
    let mut p = 0;
    while p < d.len() {
        if std::time::Instant::now() >= end {
            return false;
        }
        match s.write(&d[p..]) {
            Ok(0) => return false,
            Ok(n) => p += n,
            Err(e) if e.kind() == ErrorKind::Interrupted => {}
            Err(_) => return false,
        }
    }
    true
}
