// SPDX-License-Identifier: GPL-2.0-or-later

//! e2fsck's inode names carry no original pathname or evidence of a pending rename.
use oos_hal::update::State;
use std::fs::{self, File};
use std::io::{self, Read};
use std::os::unix::fs::{DirEntryExt, MetadataExt};
use std::path::Path;

const MAX_TEXT: u64 = 1 << 20;
const SETUP: &str = "# OOS setup v1: key = value (oos-ui)\n";
const SETTINGS: &str = "# OOS system settings: key = value (oos-ui, System menu)\n";

pub(super) fn clean(root: &Path) -> io::Result<(usize, usize)> {
    let directory = root.join("lost+found");
    match directory.symlink_metadata() {
        Ok(meta) if meta.is_dir() => {}
        Ok(_) => return Err(io::Error::other("lost+found is not a directory")),
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok((0, 0)),
        Err(e) => return Err(e),
    }
    let mut removed = 0;
    let mut kept = 0;
    for entry in fs::read_dir(&directory)? {
        let entry = entry?;
        let name = entry.file_name();
        if numbered_inode(&name) == Some(entry.ino()) && redundant(root, &entry.path()) {
            fs::remove_file(entry.path())?;
            removed += 1;
        } else {
            kept += 1;
        }
    }
    if removed != 0 {
        File::open(directory)?.sync_all()?;
    }
    Ok((removed, kept))
}

fn numbered_inode(name: &std::ffi::OsStr) -> Option<u64> {
    let number = name.to_str()?.strip_prefix('#')?;
    if number.bytes().all(|b| b.is_ascii_digit()) {
        number.parse().ok()
    } else {
        None
    }
}

fn regular_text(path: &Path) -> Option<String> {
    let meta = path.symlink_metadata().ok()?;
    if !meta.is_file() || meta.nlink() != 1 || meta.len() > MAX_TEXT {
        return None;
    }
    let mut text = String::new();
    File::open(path).ok()?.take(MAX_TEXT + 1).read_to_string(&mut text).ok()?;
    (text.len() as u64 <= MAX_TEXT).then_some(text)
}

fn redundant(root: &Path, path: &Path) -> bool {
    let Some(text) = regular_text(path) else { return false };
    let Some(destination) = destination(&text) else { return false };
    let live = root.join(destination);
    // The same format is also used by saved setups and state backups. Require a complete live copy.
    live.parent().is_some_and(|dir| dir.symlink_metadata().is_ok_and(|m| m.is_dir()))
        && regular_text(&live).is_some_and(|live| live == text)
}

fn destination(text: &str) -> Option<&'static str> {
    if let Some(body) = text.strip_prefix(SETUP) {
        return assignments(body).then_some("oos-ui.setup");
    }
    if let Some(body) = text.strip_prefix(SETTINGS) {
        return assignments(body).then_some("system.conf");
    }
    (State::parse(text).text() == text).then_some("update/state")
}

fn assignments(text: &str) -> bool {
    !text.is_empty()
        && text.ends_with('\n')
        && !text.contains('\0')
        && text.lines().all(|line| {
            let line = line.trim();
            line.is_empty()
                || line.starts_with('#')
                || line.split_once('=').is_some_and(|(key, _)| !key.trim().is_empty())
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oos_update::test_support::TempDir;

    fn name_by_inode(path: &Path) -> std::path::PathBuf {
        let metadata = path.symlink_metadata().unwrap();
        let numbered = path.with_file_name(format!("#{}", metadata.ino()));
        fs::rename(path, &numbered).unwrap();
        numbered
    }

    #[test]
    fn removes_proven_redundant_files_and_keeps_ambiguous_recoveries() {
        let directory = TempDir::new();
        let root = directory.path();
        let found = root.join("lost+found");
        fs::create_dir(&found).unwrap();
        fs::create_dir(root.join("update")).unwrap();
        let state = State::default().text();
        let setup = format!("{SETUP}timebase = 0.001\n");
        let settings = format!("{SETTINGS}remote.ssh = off\n");
        for (name, text) in [("update/state", &state), ("oos-ui.setup", &setup), ("system.conf", &settings)] {
            fs::write(root.join(name), text).unwrap();
        }
        let distinct = format!("{SETUP}timebase = 0.002\n");
        let retained =
            [state.as_str(), &setup, &settings, &distinct, "setup from another build", &setup[..setup.len() - 1]]
                .into_iter()
                .map(|text| {
                    let path = found.join(".fixture");
                    fs::write(&path, text).unwrap();
                    name_by_inode(&path)
                })
                .collect::<Vec<_>>();
        fs::write(found.join("user.setup"), &setup).unwrap();
        let link = found.join(".link");
        std::os::unix::fs::symlink(root.join("oos-ui.setup"), &link).unwrap();
        name_by_inode(&link);
        assert_eq!(clean(root).unwrap(), (3, 5));
        assert_eq!(clean(root).unwrap(), (0, 5));
        assert_eq!(fs::read_to_string(root.join("oos-ui.setup")).unwrap(), setup);
        assert_eq!(fs::read_to_string(&retained[3]).unwrap(), distinct);
    }
}
