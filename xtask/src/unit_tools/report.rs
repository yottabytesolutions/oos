// SPDX-License-Identifier: GPL-2.0-or-later

use serde::{de::DeserializeOwned, Serialize};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub(crate) struct PendingReport<'a> {
    destination: &'a Path,
    temporary: PathBuf,
    file: File,
    published: bool,
}

impl<'a> PendingReport<'a> {
    pub(crate) fn open(destination: &'a Path) -> io::Result<Self> {
        let name = destination
            .file_name()
            .filter(|name| destination.as_os_str().as_encoded_bytes().ends_with(name.as_encoded_bytes()))
            .ok_or(io::Error::other("expected an output filename"))?;
        match fs::symlink_metadata(destination) {
            Ok(metadata) if metadata.is_file() => {
                // Validate the destination without truncating a report from an earlier run.
                File::options().write(true).open(destination)?;
            }
            Ok(_) => return Err(io::Error::other("expected a regular output file")),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        let mut temporary = name.to_owned();
        temporary.push(format!(".{}.tmp", std::process::id()));
        let temporary = destination.with_file_name(temporary);
        let file = File::create_new(&temporary)?;
        Ok(Self { destination, temporary, file, published: false })
    }

    pub(crate) fn write(mut self, report: &impl Serialize) -> io::Result<()> {
        serde_json::to_writer_pretty(&mut self.file, report)?;
        self.file.write_all(b"\n")?;
        self.file.sync_all()?;
        fs::rename(&self.temporary, self.destination)?;
        self.published = true;
        Ok(())
    }
}

impl Drop for PendingReport<'_> {
    fn drop(&mut self) {
        if !self.published {
            // A cleanup failure cannot change the prior run error or publish a partial report.
            let _ = fs::remove_file(&self.temporary);
        }
    }
}

pub(crate) fn same_keys<T, U>(
    left: &BTreeMap<String, T>,
    right: &BTreeMap<String, U>,
    kind: &str,
    left_name: &str,
    right_name: &str,
) -> Result<(), String> {
    let missing = left
        .keys()
        .filter(|key| !right.contains_key(*key))
        .map(|key| (key, right_name))
        .chain(right.keys().filter(|key| !left.contains_key(*key)).map(|key| (key, left_name)))
        .next();
    if let Some((key, label)) = missing {
        return Err(format!("missing {kind} '{key}' in {label}"));
    }
    Ok(())
}

pub(crate) fn read<T: DeserializeOwned>(path: &Path, label: &str) -> Result<T, String> {
    let data = fs::read(path).map_err(|e| format!("read {label} report: {e}"))?;
    serde_json::from_slice(&data).map_err(|e| format!("invalid {label} report: {e}"))
}
