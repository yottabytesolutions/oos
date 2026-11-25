// SPDX-License-Identifier: GPL-2.0-or-later
//! The kexec restart guard: the build a kexec install restarted for, consumed by the next ordinary start.

use crate::system::durable::{durable_remove, replace_private, Disk, Files};
use std::io;
use std::path::{Path, PathBuf};

/// Where the guard sits: beside the update state.
pub(super) fn restart_mark(state_path: &Path) -> PathBuf {
    state_path.with_file_name("kexec-restart")
}

/// Record the build before restarting for its kexec trial. The updater holds its lock.
pub fn write_restart_mark(state_path: &Path, build: &str) -> io::Result<()> {
    let path = restart_mark(state_path);
    crate::system::check_data_write(&path)?;
    write_with(&mut Disk, &path, build)
}

fn write_with(files: &mut impl Files, path: &Path, build: &str) -> io::Result<()> {
    replace_private(files, path, format!("{}\n", build.trim()).as_bytes())
}

/// The recorded build, or `None` without a guard.
pub fn read_restart_mark(state_path: &Path) -> io::Result<Option<String>> {
    match std::fs::read_to_string(restart_mark(state_path)) {
        Ok(build) => Ok(Some(build)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// Consume the guard. The removal is synced: a guard that reappears after a power cut would make the next cold
/// start look like a restart this system initiated.
pub(super) fn remove_restart_mark(state_path: &Path) -> io::Result<()> {
    let path = restart_mark(state_path);
    crate::system::check_data_write(&path)?;
    durable_remove(&mut Disk, &path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closed_data_refuses_restart_mark_writes_and_removals() {
        use crate::system::admission_test::{assert_refused, with_closed_data};
        with_closed_data(module_path!(), "closed_data_refuses_restart_mark_writes_and_removals", |data, _| {
            let state = data.join("state");
            let mark = restart_mark(&state);
            std::fs::write(&mark, "previous").unwrap();
            assert_refused(write_restart_mark(&state, "next").unwrap_err());
            assert_refused(remove_restart_mark(&state).unwrap_err());
            assert_eq!(std::fs::read_to_string(mark).unwrap(), "previous");
        });
    }

    #[test]
    fn the_restart_mark_sits_beside_the_state() {
        assert_eq!(restart_mark(Path::new("/oos/update/state")), Path::new("/oos/update/kexec-restart"));
    }

    #[test]
    fn a_cut_keeps_a_complete_restart_marker() {
        let path = Path::new("update/kexec-restart");
        crate::system::durable::tests::assert_cuts(path, b"next-build\n", |files| {
            write_with(files, path, "next-build\n")
        });
    }
}
