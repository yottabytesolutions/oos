// SPDX-License-Identifier: GPL-2.0-or-later

use std::path::{Path, PathBuf};

pub(crate) struct TempDir(PathBuf);

impl TempDir {
    pub(crate) fn new() -> Self {
        let output = std::process::Command::new("mktemp").arg("-d").output().unwrap();
        assert!(output.status.success());
        Self(PathBuf::from(String::from_utf8(output.stdout).unwrap().trim_end_matches('\n')))
    }

    pub(crate) fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) {
            eprintln!("remove test directory: {error}");
        }
    }
}
