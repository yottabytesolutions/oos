// SPDX-License-Identifier: GPL-2.0-or-later
//! Durable replacement of one file in an existing directory.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

pub(crate) trait Files {
    type Staged;

    fn create(&mut self, path: &Path, options: &OpenOptions) -> io::Result<Self::Staged>;
    fn write(&mut self, file: &mut Self::Staged, bytes: &[u8]) -> io::Result<()>;
    fn sync_file(&mut self, file: Self::Staged) -> io::Result<()>;
    fn rename(&mut self, from: &Path, to: &Path) -> io::Result<()>;
    fn sync_dir(&mut self, path: &Path) -> io::Result<()>;
    fn remove(&mut self, path: &Path) -> io::Result<()>;
}

pub(crate) struct Disk;

impl Files for Disk {
    type Staged = File;

    fn create(&mut self, path: &Path, options: &OpenOptions) -> io::Result<File> {
        options.open(path)
    }

    fn write(&mut self, file: &mut File, bytes: &[u8]) -> io::Result<()> {
        file.write_all(bytes)
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

    fn remove(&mut self, path: &Path) -> io::Result<()> {
        fs::remove_file(path)
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
    let mut file = files.create(temporary, options)?;
    let staged =
        files.write(&mut file, bytes).and_then(|()| files.sync_file(file)).and_then(|()| files.rename(temporary, path));
    if let Err(error) = staged {
        remove_staged(files, temporary)
            .map_err(|cleanup| io::Error::other(format!("{error}; removing staging file: {cleanup}")))?;
        return Err(error);
    }
    files.sync_dir(dir)
}

fn remove_staged(files: &mut impl Files, path: &Path) -> io::Result<()> {
    match files.remove(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        result => result,
    }
}

/// For a serialized writer. Discard a stale staging file and create a private one.
pub(crate) fn replace_private(files: &mut impl Files, path: &Path, bytes: &[u8]) -> io::Result<()> {
    let temporary = path.with_added_extension("new");
    remove_staged(files, &temporary)?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    replace(files, path, &temporary, bytes, &options)
}

#[cfg(test)]
pub(crate) mod tests;
