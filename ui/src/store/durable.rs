// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! The persistence boundary for startup markers, setups and user files.
use std::io::{self, Read, Write};
use std::path::Path;

pub(crate) trait Files {
    fn prepare(&mut self, path: &Path) -> io::Result<()>;
    fn entries(&self, path: &Path) -> io::Result<Vec<std::path::PathBuf>>;
    fn read(&self, path: &Path) -> io::Result<String>;
    fn read_limited(&self, path: &Path, limit: usize) -> io::Result<String>;
    fn write(&mut self, path: &Path, text: &str) -> io::Result<()>;
    fn sync_file(&mut self, path: &Path) -> io::Result<()>;
    fn rename(&mut self, from: &Path, to: &Path) -> io::Result<()>;
    fn sync_dir(&mut self, path: &Path) -> io::Result<()>;
    fn remove(&mut self, path: &Path) -> io::Result<()>;
}

pub(crate) struct Disk;

impl Files for Disk {
    fn prepare(&mut self, path: &Path) -> io::Result<()> {
        super::check_target(path)?;
        super::check_target(&temporary(path))?;
        std::fs::create_dir_all(parent(path)?)
    }
    fn entries(&self, path: &Path) -> io::Result<Vec<std::path::PathBuf>> {
        std::fs::read_dir(path)?.map(|entry| entry.map(|e| e.path())).collect()
    }
    fn read(&self, path: &Path) -> io::Result<String> {
        std::fs::read_to_string(path)
    }
    fn read_limited(&self, path: &Path, limit: usize) -> io::Result<String> {
        let mut text = String::new();
        std::fs::File::open(path)?.take(limit as u64 + 1).read_to_string(&mut text)?;
        if text.len() > limit {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "setup recall exceeds byte limit"));
        }
        Ok(text.into_boxed_str().into_string())
    }
    fn write(&mut self, path: &Path, text: &str) -> io::Result<()> {
        std::fs::File::create(path)?.write_all(text.as_bytes())
    }
    fn sync_file(&mut self, path: &Path) -> io::Result<()> {
        std::fs::File::open(path)?.sync_all()
    }
    fn rename(&mut self, from: &Path, to: &Path) -> io::Result<()> {
        std::fs::rename(from, to)
    }
    fn sync_dir(&mut self, path: &Path) -> io::Result<()> {
        std::fs::File::open(path)?.sync_all()
    }
    fn remove(&mut self, path: &Path) -> io::Result<()> {
        match std::fs::remove_file(path) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            other => other,
        }
    }
}

/// The new file must be on the same filesystem as the destination. Return success only after the
/// directory entry is durable. A cut before then may expose either version, each complete.
pub(crate) fn replace(fs: &mut impl Files, path: &Path, text: &str) -> io::Result<()> {
    let temp = temporary(path);
    fs.write(&temp, text)?;
    fs.sync_file(&temp)?;
    fs.rename(&temp, path)?;
    fs.sync_dir(parent(path)?)
}

pub(super) fn write(path: &Path, data: &[u8]) -> io::Result<()> {
    if path.starts_with("/oos") {
        replace_bytes(path, data)
    } else {
        std::fs::write(path, data)
    }
}

fn replace_bytes(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut fs = Disk;
    let temp = temporary(path);
    super::check_target(&temp)?;
    let mut file = std::fs::File::create(&temp)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    fs.rename(&temp, path)?;
    fs.sync_dir(parent(path)?)
}

fn temporary(path: &Path) -> std::path::PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(".new");
    name.into()
}

pub(crate) fn remove(fs: &mut impl Files, path: &Path) -> io::Result<()> {
    fs.remove(path)?;
    fs.sync_dir(parent(path)?)
}

pub(crate) fn parent(path: &Path) -> io::Result<&Path> {
    path.parent().ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "file needs a parent directory"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_staging_error_keeps_the_existing_binary_file() {
        let dir = std::env::temp_dir().join(format!("oos-durable-bytes-{}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("reference.bin");
        std::fs::write(&path, b"previous").unwrap();
        let temp = temporary(&path);
        std::fs::create_dir(&temp).unwrap();
        assert!(replace_bytes(&path, b"next\0binary").is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"previous");
        std::fs::remove_dir(&temp).unwrap();
        replace_bytes(&path, b"next\0binary").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"next\0binary");
        assert!(!temp.exists());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
