// SPDX-License-Identifier: GPL-2.0-or-later
//! Bounded responder teardown.
use super::*;

pub(super) fn flush(r: &mut Responder) {
    for line in r.log.drain(..) {
        eprintln!("oos-ui: {}", line);
    }
}

/// A coalescing control wakeup, polled beside the network sockets.
pub(super) struct Wake {
    #[cfg(unix)]
    socket: std::os::unix::net::UnixDatagram,
}

impl Wake {
    pub(super) fn pair() -> std::io::Result<(Self, Self)> {
        #[cfg(unix)]
        {
            let (tx, rx) = std::os::unix::net::UnixDatagram::pair()?;
            tx.set_nonblocking(true)?;
            rx.set_nonblocking(true)?;
            Ok((Self { socket: tx }, Self { socket: rx }))
        }
        #[cfg(not(unix))]
        Ok((Self {}, Self {}))
    }

    pub(super) fn notify(&self) {
        #[cfg(unix)]
        // A full socket already has a pending wakeup; the control channel owns the messages.
        let _ = self.socket.send(&[1]);
    }

    #[cfg(unix)]
    pub(super) fn fd(&self) -> std::os::fd::RawFd {
        use std::os::fd::AsRawFd;
        self.socket.as_raw_fd()
    }

    #[cfg(unix)]
    pub(super) fn drain(&self) {
        while self.socket.recv(&mut [0]).is_ok() {}
    }
}

impl Mdns {
    pub fn new(only: Option<Vec<String>>) -> Mdns {
        Mdns { tx: None, th: None, wake: None, only }
    }

    /// The configuration to announce (on or off).
    pub fn set(&mut self, cfg: Config) {
        if self.tx.is_none() {
            if !cfg.on {
                return;
            }
            let (tx, rx) = std::sync::mpsc::channel();
            let (wake, receiver) = match lifecycle::Wake::pair() {
                Ok(pair) => pair,
                Err(e) => {
                    eprintln!("oos-ui: mDNS: control wakeup: {}", e);
                    return;
                }
            };
            let only = self.only.clone();
            match std::thread::Builder::new().name("mdns".into()).spawn(move || run(rx, only, receiver)) {
                Ok(h) => {
                    self.tx = Some(tx);
                    self.th = Some(h);
                    self.wake = Some(wake);
                }
                Err(e) => {
                    eprintln!("oos-ui: mDNS: no thread: {}", e);
                    return;
                }
            }
        }
        if let Some(tx) = &self.tx {
            let _ = tx.send(Ctl::Set(cfg));
            if let Some(wake) = &self.wake {
                wake.notify();
            }
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    };
    use std::time::Instant;

    #[test]
    fn shutdown_wakes_network_wait_and_leaves_time_for_http() {
        let (tx, rx) = mpsc::channel();
        let (wake, receiver) = Wake::pair().unwrap();
        let (ready, waiting) = mpsc::channel();
        let stopped = Arc::new(AtomicBool::new(false));
        let finished = stopped.clone();
        let th = std::thread::spawn(move || {
            ready.send(()).unwrap();
            loop {
                net::wait(Vec::new(), 1.0, &receiver);
                if matches!(rx.try_recv(), Ok(Ctl::Stop)) {
                    finished.store(true, Ordering::Release);
                    break;
                }
            }
        });
        let mut mdns = Mdns { tx: Some(tx), th: Some(th), wake: Some(wake), only: None };
        let (close_http, http_closed) = mpsc::channel();
        let http = std::thread::spawn(move || {
            http_closed.recv().unwrap();
        });
        waiting.recv_timeout(crate::ui::lifecycle::test_wait::HANG_GUARD).expect("mDNS worker entering network wait");
        let deadline = Instant::now() + crate::ui::lifecycle::test_wait::HANG_GUARD;
        mdns.shutdown(deadline);
        let mdns_stopped = stopped.load(Ordering::Acquire);
        close_http.send(()).unwrap();
        let http_pending = crate::system::shutdown::join(vec![http], deadline);
        assert!(mdns_stopped, "idle mDNS must exit within the remote budget");
        assert_eq!(http_pending, 0, "HTTP shares the remote deadline");
        mdns.shutdown(deadline);
    }
}

impl Mdns {
    pub fn shutdown(&mut self, deadline: std::time::Instant) {
        if let Some(tx) = self.tx.take() {
            let _ = tx.send(Ctl::Stop);
        }
        if let Some(wake) = self.wake.take() {
            wake.notify();
        }
        if let Some(h) = self.th.take() {
            crate::system::shutdown::join(vec![h], deadline);
        }
    }
}
