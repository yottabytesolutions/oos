// SPDX-License-Identifier: GPL-2.0-or-later
use super::io;
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
    if let Some(d) = state_path.parent() {
        std::fs::create_dir_all(d).map_err(io("creating the update state directory"))?;
    }
    let f = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(state_path.with_extension("lock"))
        .map_err(io("opening the update lock"))?;
    #[cfg(unix)]
    {
        use std::os::fd::AsRawFd;
        let flags = libc::LOCK_EX | if nonblocking { libc::LOCK_NB } else { 0 };
        // SAFETY: f owns the open lock file descriptor for the duration of this call.
        if unsafe { libc::flock(f.as_raw_fd(), flags) } != 0 {
            return Err(format!("locking the update state: {}", std::io::Error::last_os_error()));
        }
    }
    Ok(Lock { _file: f })
}
