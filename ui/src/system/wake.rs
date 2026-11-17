// SPDX-License-Identifier: GPL-2.0-or-later
//! Nonblocking, coalescing notification polled beside network sockets or input.
use std::io;
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::net::UnixDatagram;

pub struct Wake {
    sender: UnixDatagram,
    receiver: UnixDatagram,
}

impl Wake {
    pub fn new() -> io::Result<Self> {
        let (sender, receiver) = UnixDatagram::pair()?;
        sender.set_nonblocking(true)?;
        receiver.set_nonblocking(true)?;
        Ok(Self { sender, receiver })
    }

    pub fn notify(&self) {
        // A full socket already signals pending work; the caller owns the messages.
        while let Err(e) = self.sender.send(&[1]) {
            if e.kind() != io::ErrorKind::Interrupted {
                break;
            }
        }
    }

    pub fn fd(&self) -> RawFd {
        self.receiver.as_raw_fd()
    }

    pub fn drain(&self) {
        loop {
            match self.receiver.recv(&mut [0]) {
                Ok(_) => {}
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(_) => break,
            }
        }
    }

    pub fn wait(&self, timeout_ms: i32) -> bool {
        let mut fd = libc::pollfd { fd: self.fd(), events: libc::POLLIN, revents: 0 };
        // SAFETY: fd is initialized and the owned socket stays open for the call.
        let ready = unsafe { libc::poll(&mut fd, 1, timeout_ms) };
        ready > 0 && fd.revents & libc::POLLIN != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drain_clears_all_notifications_and_allows_rearming() {
        let wake = Wake::new().unwrap();
        assert!(!wake.wait(0));
        wake.drain();
        for _ in 0..8 {
            wake.notify();
        }
        assert!(wake.wait(0));
        wake.drain();
        assert!(!wake.wait(0));
        wake.notify();
        assert!(wake.wait(0));
        wake.drain();
        assert!(!wake.wait(0));
    }
}
