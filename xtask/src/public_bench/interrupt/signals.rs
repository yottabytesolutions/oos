// SPDX-License-Identifier: GPL-2.0-or-later

use std::{
    cell::Cell,
    io,
    mem::MaybeUninit,
    ptr,
    sync::mpsc::{self, Receiver, RecvTimeoutError},
    thread,
    time::Duration,
};

const SIGNALS: [libc::c_int; 3] = [libc::SIGINT, libc::SIGTERM, libc::SIGHUP];

/// Listen for a CLI's first termination signal while its main thread can clean up.
#[derive(Debug)]
pub(super) struct TerminationSignals {
    received: Receiver<io::Result<u8>>,
    exit_code: Cell<Option<u8>>,
}

impl TerminationSignals {
    /// Call on the CLI's main thread before it spawns any workers. The main
    /// thread and the threads it spawns leave these signals blocked until exit.
    pub(super) fn listen_until_exit() -> io::Result<Self> {
        let set = signal_set()?;
        block_signals(&set)?;
        for signal in SIGNALS {
            // SAFETY: these are valid signals and SIG_DFL installs no callback.
            // They are already blocked. Reset inherited ignores so the OS queues them.
            if unsafe { libc::signal(signal, libc::SIG_DFL) } == libc::SIG_ERR {
                return Err(io::Error::last_os_error());
            }
        }
        let (send, received) = mpsc::channel();
        thread::Builder::new().name("termination-signals".into()).spawn(move || {
            let _ = send.send(wait_for_signal(&set));
        })?;
        Ok(Self { received, exit_code: Cell::new(None) })
    }

    /// Wait for the first signal and return its shell exit code, remembering it
    /// for future calls. A zero timeout checks for a signal without blocking.
    pub(super) fn wait(&self, timeout: Duration) -> io::Result<Option<u8>> {
        if let Some(code) = self.exit_code.get() {
            return Ok(Some(code));
        }
        match self.received.recv_timeout(timeout) {
            Ok(result) => {
                let code = result?;
                self.exit_code.set(Some(code));
                Ok(Some(code))
            }
            Err(RecvTimeoutError::Timeout) => Ok(None),
            Err(RecvTimeoutError::Disconnected) => Err(io::Error::other("termination signal listener stopped")),
        }
    }
}

fn signal_set() -> io::Result<libc::sigset_t> {
    let mut set = MaybeUninit::<libc::sigset_t>::uninit();
    // SAFETY: sigemptyset initializes the writable local set. Every added signal is valid.
    unsafe {
        if libc::sigemptyset(set.as_mut_ptr()) != 0 {
            return Err(io::Error::last_os_error());
        }
        for signal in SIGNALS {
            if libc::sigaddset(set.as_mut_ptr(), signal) != 0 {
                return Err(io::Error::last_os_error());
            }
        }
        Ok(set.assume_init())
    }
}

fn block_signals(set: &libc::sigset_t) -> io::Result<()> {
    // SAFETY: pthread_sigmask copies the initialized set during the call.
    let status = unsafe { libc::pthread_sigmask(libc::SIG_BLOCK, set, ptr::null_mut()) };
    if status == 0 {
        Ok(())
    } else {
        Err(io::Error::from_raw_os_error(status))
    }
}

fn wait_for_signal(set: &libc::sigset_t) -> io::Result<u8> {
    let mut signal: libc::c_int = 0;
    loop {
        // SAFETY: the signals are blocked on this thread and both pointers remain valid for the call.
        match unsafe { libc::sigwait(set, &mut signal) } {
            0 => return u8::try_from(128 + signal).map_err(io::Error::other),
            libc::EINTR => continue,
            error => return Err(io::Error::from_raw_os_error(error)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn receives_termination_signals_without_running_a_handler() {
        thread::spawn(|| {
            let set = signal_set().unwrap();
            block_signals(&set).unwrap();
            for (signal, code) in [(libc::SIGINT, 130), (libc::SIGTERM, 143), (libc::SIGHUP, 129)] {
                // SAFETY: raise targets this thread, which has blocked the signal.
                assert_eq!(unsafe { libc::raise(signal) }, 0);
                assert_eq!(wait_for_signal(&set).unwrap(), code);
            }
        })
        .join()
        .unwrap();
    }

    #[test]
    fn remembers_the_received_signal_during_cleanup() {
        let (send, received) = mpsc::channel();
        let signals = TerminationSignals { received, exit_code: Cell::new(None) };
        assert_eq!(signals.wait(Duration::ZERO).unwrap(), None);
        send.send(Ok(143)).unwrap();
        drop(send);
        assert_eq!(signals.wait(Duration::ZERO).unwrap(), Some(143));
        assert_eq!(signals.wait(Duration::ZERO).unwrap(), Some(143));
    }
}
