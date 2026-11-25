// SPDX-License-Identifier: GPL-2.0-or-later
use std::{io, path::Path};

/// Exclusive lock on the update state: rcS, the watcher, the GUI job and a shell call never interleave their
/// read-modify-write of the state and the card (held until dropped).
#[derive(Debug)]
pub struct Lock {
    _file: std::fs::File,
}

pub fn lock(state_path: &Path) -> Result<Lock, String> {
    lock_with(state_path, false)
}

pub(super) fn lock_with(state_path: &Path, nonblocking: bool) -> Result<Lock, String> {
    acquire(state_path, nonblocking).map_err(|error| error.to_string())
}

/// Acquire the update lock without waiting for an install or restage.
pub fn try_lock(state_path: &Path) -> io::Result<Lock> {
    acquire(state_path, true)
}

pub(super) fn acquire(state_path: &Path, nonblocking: bool) -> io::Result<Lock> {
    lock_file(state_path, nonblocking, "update state")
}

pub(super) fn lock_file(path: &Path, nonblocking: bool, subject: &str) -> io::Result<Lock> {
    crate::system::check_data_write(path)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let file =
        std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(path.with_extension("lock"))?;
    #[cfg(unix)]
    {
        use std::os::fd::AsRawFd;
        let flags = libc::LOCK_EX | if nonblocking { libc::LOCK_NB } else { 0 };
        // SAFETY: file owns the open lock file descriptor for the duration of this call.
        if unsafe { libc::flock(file.as_raw_fd(), flags) } != 0 {
            let error = io::Error::last_os_error();
            let message = if error.kind() == io::ErrorKind::WouldBlock {
                format!("{subject} is busy ({error})")
            } else {
                format!("locking the {subject} lock: {error}")
            };
            return Err(io::Error::new(error.kind(), message));
        }
    }
    // An operation already holding the writable descriptor may finish. A queued one
    // must recheck admission after the previous holder releases the lock.
    crate::system::check_data_write(path)?;
    Ok(Lock { _file: file })
}
