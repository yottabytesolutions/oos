// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

use super::device::Port;
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::time::{Duration, Instant};

const REQUEST: &str = "/tmp/oos-watchdog.handoff";

pub(super) struct Native;

impl Port for Native {
    type Handle = File;

    fn present(&self) -> io::Result<bool> {
        std::path::Path::new("/dev/watchdog").try_exists()
    }

    fn pid(&self) -> io::Result<String> {
        std::fs::read_to_string("/tmp/oos-watchdog.pid")
    }

    fn cmdline(&self, pid: libc::pid_t) -> io::Result<Vec<u8>> {
        std::fs::read(format!("/proc/{pid}/cmdline"))
    }

    fn requester(&self) -> io::Result<Option<String>> {
        match std::fs::read_to_string(REQUEST) {
            Ok(pid) => Ok(Some(pid)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    fn request(&self) -> io::Result<()> {
        let mut file = OpenOptions::new().write(true).create_new(true).open(REQUEST)?;
        if let Err(e) = writeln!(file, "{}", std::process::id()) {
            // We created it, but cannot safely hand off without a recorded requester.
            if let Err(cleanup) = std::fs::remove_file(REQUEST) {
                self.report(&format!("cannot remove incomplete watchdog handoff: {cleanup}"));
            }
            return Err(e);
        }
        Ok(())
    }

    fn remove_request(&self) -> io::Result<()> {
        std::fs::remove_file(REQUEST)
    }

    fn signal(&self, pid: libc::pid_t, signal: libc::c_int) -> io::Result<()> {
        // SAFETY: Device validates positive pids above 1; signal is SIGUSR1 or the nonmutating existence check 0.
        if unsafe { libc::kill(pid, signal) } == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }

    fn open(&self) -> io::Result<File> {
        OpenOptions::new().write(true).open("/dev/watchdog")
    }

    fn now(&self) -> Instant {
        Instant::now()
    }

    fn pause(&self) {
        std::thread::sleep(Duration::from_millis(10));
    }

    fn report(&self, message: &str) {
        eprintln!("oos-kexec: {message}");
        if let Err(e) =
            OpenOptions::new().write(true).open("/dev/kmsg").and_then(|mut file| writeln!(file, "oos-kexec: {message}"))
        {
            eprintln!("oos-kexec: /dev/kmsg diagnostic write failed: {e}");
        }
    }
}
