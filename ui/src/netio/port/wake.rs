// SPDX-License-Identifier: GPL-2.0-or-later
//! Nonblocking, coalescing notification polled beside network sockets or input.
use std::io;
use std::os::fd::{AsFd, AsRawFd, RawFd};
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
        while self.sender.send(&[1]).is_err_and(|e| e.kind() == io::ErrorKind::Interrupted) {}
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
        oos_hal::system::wait_readable(self.receiver.as_fd(), timeout_ms)
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
