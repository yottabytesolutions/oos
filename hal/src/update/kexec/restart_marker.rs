// SPDX-License-Identifier: GPL-2.0-or-later

use crate::system::durable::{replace_private, Disk, Files};
use std::io;
use std::path::Path;

/// Record the build before restarting for its kexec trial. The updater holds its lock.
pub fn write_restart_marker(path: &Path, build: &str) -> io::Result<()> {
    write_with(&mut Disk, path, build)
}

fn write_with(files: &mut impl Files, path: &Path, build: &str) -> io::Result<()> {
    replace_private(files, path, format!("{}\n", build.trim()).as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cut_keeps_a_complete_restart_marker() {
        let path = Path::new("update/kexec-restart");
        crate::system::durable::tests::assert_cuts(path, b"next-build\n", |files| {
            write_with(files, path, "next-build\n")
        });
    }
}
