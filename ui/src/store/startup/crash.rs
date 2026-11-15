// SPDX-License-Identifier: GPL-2.0-or-later
//! Read the most recent completed crash archive, bounded to one short header.

use std::io::{self, Read};
use std::path::Path;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Crash {
    time: String,
    first_line: String,
}

impl Crash {
    pub(crate) fn text(&self) -> String {
        format!("{} {}", self.time, self.first_line)
    }
}

pub(crate) fn latest(root: &Path) -> io::Result<Option<Crash>> {
    let mut newest = None;
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        let mut parts = name.split('-');
        let Some(sequence) = parts.next().and_then(|s| s.parse::<u64>().ok()) else { continue };
        let Some(time) = parts.next().filter(|s| valid_time(s)) else { continue };
        let Some(sum) = parts.next().filter(|s| s.len() == 12 && s.bytes().all(|b| b.is_ascii_hexdigit())) else {
            continue;
        };
        let _ = sum;
        if parts.next().is_some() || newest.as_ref().is_some_and(|(n, _)| *n >= sequence) {
            continue;
        }
        if let Some(first_line) = header(&entry.path())? {
            newest = Some((sequence, Crash { time: time.into(), first_line }));
        }
    }
    Ok(newest.map(|(_, crash)| crash))
}

fn valid_time(s: &str) -> bool {
    s.len() == 16
        && s.as_bytes()[8] == b'T'
        && s.ends_with('Z')
        && s.bytes().enumerate().all(|(i, b)| i == 8 || i == 15 || b.is_ascii_digit())
}

fn header(dir: &Path) -> io::Result<Option<String>> {
    let mut records = std::fs::read_dir(dir)?.filter_map(Result::ok).collect::<Vec<_>>();
    records.sort_by_key(|e| e.file_name());
    for entry in records {
        if !entry.file_name().to_string_lossy().starts_with("dmesg-ramoops-") || !entry.file_type()?.is_file() {
            continue;
        }
        let mut bytes = Vec::new();
        std::fs::File::open(entry.path())?.take(256).read_to_end(&mut bytes)?;
        let first = bytes.split(|b| *b == b'\n').next().unwrap_or_default();
        let line: String = String::from_utf8_lossy(first).chars().filter(|c| !c.is_control()).collect();
        if line.starts_with("Panic") || line.starts_with("Oops") {
            return Ok(Some(line));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latest_completed_archive_uses_numeric_order_and_bounds_header() {
        let root = std::env::temp_dir().join(format!("oos-crash-reader-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        for (name, text) in [
            ("9-20260101T000000Z-abcdef012345", "Oops old\n"),
            ("10-20260102T000000Z-abcdef012345", "Panic new\nprivate body"),
            ("11-20260103T000000Z-abcdef012345", "console only\n"),
            (".partial-12", "Panic incomplete\n"),
            (".tmp", "Panic pending\n"),
        ] {
            let dir = root.join(name);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("dmesg-ramoops-0"), text).unwrap();
        }
        assert_eq!(latest(&root).unwrap().unwrap().text(), "20260102T000000Z Panic new");
        std::fs::write(
            root.join("10-20260102T000000Z-abcdef012345/dmesg-ramoops-0"),
            format!("Panic {}", "x".repeat(1000)),
        )
        .unwrap();
        assert!(latest(&root).unwrap().unwrap().first_line.len() <= 256);
        std::fs::remove_dir_all(root).unwrap();
    }
}
