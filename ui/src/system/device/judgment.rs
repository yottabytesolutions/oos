// SPDX-License-Identifier: GPL-2.0-or-later

use std::fs::File;
use std::io::{self, Read};
use std::os::fd::{AsRawFd, FromRawFd};

pub(super) struct JudgmentWatch(File);

impl JudgmentWatch {
    pub(super) fn open() -> io::Result<Self> {
        // SAFETY: inotify_init1 takes flags only and returns a new descriptor on success.
        let fd = unsafe { libc::inotify_init1(libc::IN_NONBLOCK | libc::IN_CLOEXEC) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: fd is the new, unowned descriptor returned above.
        let file = unsafe { File::from_raw_fd(fd) };
        // SAFETY: fd is live and the directory name is NUL-terminated for the duration of the call.
        let watch = unsafe { libc::inotify_add_watch(fd, c"/tmp".as_ptr(), libc::IN_MOVED_TO) };
        if watch < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self(file))
    }

    pub(super) fn fd(&self) -> i32 {
        self.0.as_raw_fd()
    }

    pub(super) fn drain(&mut self) {
        if let Err(error) = self.0.read(&mut [0; 4096]) {
            if !matches!(error.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted) {
                eprintln!("oos-ui: read start-up judgment watch: {error}");
            }
        }
    }
}
