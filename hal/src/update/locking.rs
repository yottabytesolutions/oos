// SPDX-License-Identifier: GPL-2.0-or-later
use std::path::Path;

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
    lock_file(state_path, nonblocking, "update state")
}

pub(super) fn lock_file(path: &Path, nonblocking: bool, subject: &str) -> Result<Lock, String> {
    crate::system::check_data_write(path).map_err(|error| format!("opening the {subject} lock: {error}"))?;
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d).map_err(|error| format!("creating the {subject} lock directory: {error}"))?;
    }
    let f = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path.with_extension("lock"))
        .map_err(|error| format!("opening the {subject} lock: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::fd::AsRawFd;
        let flags = libc::LOCK_EX | if nonblocking { libc::LOCK_NB } else { 0 };
        // SAFETY: f owns the open lock file descriptor for the duration of this call.
        if unsafe { libc::flock(f.as_raw_fd(), flags) } != 0 {
            return Err(format!("locking the {subject} lock: {}", std::io::Error::last_os_error()));
        }
    }
    // An operation already holding the writable descriptor may finish. A queued one
    // must recheck admission after the previous holder releases the lock.
    crate::system::check_data_write(path).map_err(|error| format!("locking the {subject} lock: {error}"))?;
    Ok(Lock { _file: f })
}
