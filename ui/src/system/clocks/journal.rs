// SPDX-License-Identifier: GPL-2.0-or-later
//! Same-boot recovery receipt. Rename commits every saved value before sysfs is changed.
use super::{Floor, Kind};
use std::io::{self, Write};
use std::path::PathBuf;

pub(super) struct Journal(PathBuf);
impl Journal {
    pub(super) fn new(path: PathBuf) -> Self {
        Self(path)
    }

    pub(super) fn load(&self) -> io::Result<Option<Vec<(Kind, u64)>>> {
        let text = match std::fs::read_to_string(&self.0) {
            Ok(text) => text,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e),
        };
        let mut values = Vec::new();
        for line in text.lines() {
            let fields: Vec<_> = line.split_whitespace().collect();
            let [name, value] = fields.as_slice() else {
                return Err(io::Error::other("invalid clock recovery receipt"));
            };
            let kind = name.parse::<Kind>()?;
            if values.iter().any(|(saved, _)| *saved == kind) {
                return Err(io::Error::other("invalid clock recovery receipt"));
            }
            values.push((kind, value.parse().map_err(io::Error::other)?));
        }
        if values.is_empty() {
            return Err(io::Error::other("empty clock recovery receipt"));
        }
        Ok(Some(values))
    }

    pub(super) fn save(&self, floors: &[Floor], index: usize, previous: u64) -> io::Result<()> {
        let text: String = floors
            .iter()
            .enumerate()
            .filter_map(|(i, floor)| {
                let saved = if i == index { Some(previous) } else { floor.saved() };
                saved.map(|value| format!("{} {value}\n", floor.kind))
            })
            .collect();
        let temp = self.0.with_extension("new");
        // Remove stale staging entries without following links, then claim a fresh file.
        match std::fs::remove_file(&temp) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
            _ => {}
        }
        let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(&temp)?;
        file.write_all(text.as_bytes())?;
        // tmpfs survives process death, not a reboot; no disk durability is needed.
        std::fs::rename(temp, &self.0)
    }

    pub(super) fn clear(&self) -> io::Result<()> {
        match std::fs::remove_file(&self.0) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            result => result,
        }
    }
}
