// SPDX-License-Identifier: GPL-2.0-or-later

//! What a reader thread holds or answers before a body: the one large-body place and `100 Continue`.

use super::{Head, LARGE_BODY};
use std::io::Write;
use std::net::TcpStream;
use std::time::Duration;

/// The large-body place of a reader, given back when the reader ends.
pub(super) struct LargeSlot<'a>(Option<&'a std::sync::atomic::AtomicBool>);

impl<'a> LargeSlot<'a> {
    /// One large body at a time: a second is answered at its head, so the readers hold at most one. None: taken.
    pub(super) fn take(h: &Head, taken: &'a std::sync::atomic::AtomicBool) -> Option<Self> {
        let large = h.len.is_some_and(|n| n > LARGE_BODY);
        if large && taken.swap(true, std::sync::atomic::Ordering::SeqCst) {
            return None;
        }
        Some(Self(large.then_some(taken)))
    }
}

impl Drop for LargeSlot<'_> {
    fn drop(&mut self) {
        if let Some(b) = self.0 {
            b.store(false, std::sync::atomic::Ordering::SeqCst);
        }
    }
}

/// `100 Continue` for a client that waits for it before its body; false when the connection is gone.
pub(super) fn send_continue(s: &TcpStream) -> bool {
    let _ = s.set_write_timeout(Some(Duration::from_secs(1)));
    let mut w = s;
    w.write_all(b"HTTP/1.1 100 Continue\r\n\r\n").is_ok()
}
