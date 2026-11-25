// SPDX-License-Identifier: GPL-2.0-or-later
//! Durable replacement of one file in an existing directory.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

pub(crate) trait Files {
    type Staged;

    fn write(&mut self, path: &Path, bytes: &[u8], options: &OpenOptions) -> io::Result<Self::Staged>;
    fn sync_file(&mut self, file: Self::Staged) -> io::Result<()>;
    fn rename(&mut self, from: &Path, to: &Path) -> io::Result<()>;
    fn sync_dir(&mut self, path: &Path) -> io::Result<()>;
}

pub(crate) struct Disk;

impl Files for Disk {
    type Staged = File;

    fn write(&mut self, path: &Path, bytes: &[u8], options: &OpenOptions) -> io::Result<File> {
        let mut file = options.open(path)?;
        file.write_all(bytes)?;
        Ok(file)
    }

    fn sync_file(&mut self, file: File) -> io::Result<()> {
        file.sync_all()
    }

    fn rename(&mut self, from: &Path, to: &Path) -> io::Result<()> {
        fs::rename(from, to)
    }

    fn sync_dir(&mut self, path: &Path) -> io::Result<()> {
        File::open(path)?.sync_all()
    }
}

/// Publish a complete file after syncing its staged contents, then sync the renamed entry.
pub(crate) fn durable_replace(path: &Path, temporary: &Path, bytes: &[u8], options: &OpenOptions) -> io::Result<()> {
    replace(&mut Disk, path, temporary, bytes, options)
}

fn replace(
    files: &mut impl Files,
    path: &Path,
    temporary: &Path,
    bytes: &[u8],
    options: &OpenOptions,
) -> io::Result<()> {
    let dir = path.parent().filter(|dir| !dir.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let file = files.write(temporary, bytes, options)?;
    files.sync_file(file)?;
    files.rename(temporary, path)?;
    files.sync_dir(dir)
}

/// For a serialized writer. Reuse a staging file left by a cut and keep the new copy private.
pub(crate) fn replace_private(files: &mut impl Files, path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut temporary = path.as_os_str().to_os_string();
    temporary.push(".new");
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    replace(files, path, Path::new(&temporary), bytes, &options)
}

#[cfg(test)]
pub(crate) mod tests;
