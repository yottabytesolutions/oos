// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! Calibration file container of the stock system (docs/protocols/self-cal.md 7.1): u32 CRC32 of the
//! 20-byte header, u32 0x14, header (version 0, time, date, payload length, CRC32 of the payload), payload. OOS reads
//! the factory files in /rigol/data and writes its own results with the same layout to its own directory (the /oos
//! data partition), atomically (temporary file, fsync, rename; the previous file kept as .bak). Paths under /rigol
//! are refused.

use crate::crc32;

/// Directory of OOS's own calibration files.
pub const OOS_CAL_DIR: &str = "/oos/cal";

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    Short,
    HeaderSize,
    HeaderCrc,
    Length,
    PayloadCrc,
}

/// Time stamp of the header: (year, month, day, hour, minute, second).
pub type Stamp = (u32, u32, u32, u32, u32, u32);

fn u32_at(d: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(d.get(o..o + 4)?.try_into().ok()?))
}

/// The payload of a container, both CRCs and the length checked.
pub fn parse(d: &[u8]) -> Result<&[u8], Error> {
    let (hcrc, hsize) = (u32_at(d, 0).ok_or(Error::Short)?, u32_at(d, 4).ok_or(Error::Short)?);
    if hsize != 0x14 {
        return Err(Error::HeaderSize);
    }
    let h = d.get(8..28).ok_or(Error::Short)?;
    if crc32(h) != hcrc {
        return Err(Error::HeaderCrc);
    }
    let len = u32_at(d, 20).ok_or(Error::Short)? as usize;
    let p = d.get(28..28 + len).ok_or(Error::Length)?;
    if crc32(p) != u32_at(d, 24).ok_or(Error::Short)? {
        return Err(Error::PayloadCrc);
    }
    Ok(p)
}

/// The header stamp of a container.
pub fn stamp(d: &[u8]) -> Option<Stamp> {
    let (t, dt) = (u32_at(d, 12)?, u32_at(d, 16)?);
    Some((dt >> 8, dt & 0xff, t >> 24, (t >> 16) & 0xff, (t >> 8) & 0xff, t & 0xff))
}

/// A container around `payload`.
pub fn build(payload: &[u8], s: Stamp) -> Vec<u8> {
    let mut h = Vec::with_capacity(20);
    for v in [0, s.2 << 24 | s.3 << 16 | s.4 << 8 | s.5, s.0 << 8 | s.1, payload.len() as u32, crc32(payload)] {
        h.extend_from_slice(&v.to_le_bytes());
    }
    let mut out = crc32(&h).to_le_bytes().to_vec();
    out.extend_from_slice(&0x14u32.to_le_bytes());
    out.extend_from_slice(&h);
    out.extend_from_slice(payload);
    out
}

/// UTC now as a stamp (the unit has no time zone set under OOS).
pub fn now() -> Stamp {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    stamp_at(secs)
}

fn stamp_at(secs: u64) -> Stamp {
    let (days, rem) = (secs / 86400, secs % 86400);
    // civil from days (Howard Hinnant's algorithm)
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + (m <= 2) as i64;
    (y as u32, m as u32, d as u32, (rem / 3600) as u32, (rem / 60 % 60) as u32, (rem % 60) as u32)
}

/// A failed save, distinguishing files already live from an unsuccessful publication.
#[derive(Debug)]
pub enum SaveError {
    Failed(std::io::Error),
    PublishedDirectorySync(std::io::Error),
}

impl From<std::io::Error> for SaveError {
    fn from(error: std::io::Error) -> Self {
        Self::Failed(error)
    }
}

impl std::fmt::Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Failed(error) => error.fmt(f),
            Self::PublishedDirectorySync(error) => {
                write!(f, "calibration files published; directory fsync failed: {}", error)
            }
        }
    }
}

impl std::error::Error for SaveError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Failed(error) | Self::PublishedDirectorySync(error) => Some(error),
        }
    }
}

/// Writes one calibration file with the same staging, backup and durability as `save_all`.
pub fn save(dir: &str, name: &str, payload: &[u8], s: Stamp) -> Result<(), SaveError> {
    save_all(dir, &[(name, payload)], s)
}

/// Stage and fsync every container before replacing any file. Each previous file is kept as `.bak`.
/// Restore current good files on a publish error, but not the previous `.bak` generation.
/// Separate renames are not atomic across power loss. `PublishedDirectorySync` means all new files are live,
/// but the final directory fsync failed; callers must reload the live files even for that error.
pub fn save_all(dir: &str, files: &[(&str, &[u8])], s: Stamp) -> Result<(), SaveError> {
    save_all_with_sync(dir, files, s, |dir| std::fs::File::open(dir)?.sync_all())
}

fn save_all_with_sync(
    dir: &str,
    files: &[(&str, &[u8])],
    s: Stamp,
    sync: impl FnOnce(&str) -> std::io::Result<()>,
) -> Result<(), SaveError> {
    let mut paths = Vec::new();
    for (name, payload) in files {
        match stage(dir, name, payload, s) {
            Ok(path) => paths.push(path),
            Err(e) => {
                cleanup(&paths);
                return Err(e.into());
            }
        }
    }
    let result = publish(&paths);
    cleanup(&paths);
    match (result, sync(dir)) {
        (Err(e), Err(sync)) => Err(std::io::Error::other(format!("{}; directory fsync: {}", e, sync)).into()),
        (Err(e), _) => Err(e.into()),
        (Ok(()), result) => result.map_err(SaveError::PublishedDirectorySync),
    }
}

fn cleanup(paths: &[String]) {
    for path in paths {
        remove_temporary(&format!("{}.new", path));
    }
}

fn publish(paths: &[String]) -> std::io::Result<()> {
    publish_with(paths, |from, to| std::fs::rename(from, to))
}

fn publish_with(paths: &[String], mut rename: impl FnMut(&str, &str) -> std::io::Result<()>) -> std::io::Result<()> {
    let mut replaced = Vec::new();
    for path in paths {
        let mut old = false;
        let result = (|| {
            old = std::path::Path::new(path).try_exists()?;
            if old {
                backup_with(path, |from, to| std::fs::hard_link(from, to))?;
            }
            rename(&format!("{}.new", path), path)
        })();
        if let Err(e) = result {
            return Err(restore(&replaced, e, &mut rename));
        }
        replaced.push((path, old));
    }
    Ok(())
}

fn backup_with(path: &str, link: impl FnOnce(&str, &str) -> std::io::Result<()>) -> std::io::Result<()> {
    let backup = format!("{}.bak", path);
    let temporary = format!("{}.new", backup);
    remove_temporary(&temporary);
    link(path, &temporary)?;
    let result = (|| {
        let dir =
            std::path::Path::new(path).parent().ok_or_else(|| std::io::Error::other("no calibration directory"))?;
        std::fs::File::open(dir)?.sync_all()?;
        std::fs::rename(&temporary, &backup)
    })();
    if result.is_err() {
        remove_temporary(&temporary);
    }
    result
}

fn restore(
    replaced: &[(&String, bool)],
    error: std::io::Error,
    rename: &mut impl FnMut(&str, &str) -> std::io::Result<()>,
) -> std::io::Error {
    let mut errors = Vec::new();
    for &(path, old) in replaced.iter().rev() {
        let result = if old { rename(&format!("{}.bak", path), path) } else { std::fs::remove_file(path) };
        if let Err(e) = result {
            errors.push(format!("restore {}: {}", path, e));
        }
    }
    if errors.is_empty() {
        error
    } else {
        std::io::Error::other(format!("{}; {}", error, errors.join("; ")))
    }
}

fn remove_temporary(path: &str) {
    if let Err(e) = std::fs::remove_file(path) {
        if e.kind() != std::io::ErrorKind::NotFound {
            eprintln!("calibration cleanup {}: {}", path, e);
        }
    }
}

fn stage(dir: &str, name: &str, payload: &[u8], s: Stamp) -> std::io::Result<String> {
    use std::io::Write;
    let refuse = |m: &str| std::io::Error::new(std::io::ErrorKind::PermissionDenied, m.to_string());
    let canon = std::path::Path::new(dir).components().collect::<std::path::PathBuf>();
    if canon.starts_with("/rigol") || dir.contains("..") || name.contains('/') {
        return Err(refuse("calibration files are never written under /rigol"));
    }
    // without the data partition, /oos is a directory of the initramfs: the file would be gone at the next start
    if cfg!(target_os = "linux")
        && canon.starts_with("/oos")
        && !std::fs::read_to_string("/proc/mounts").is_ok_and(|m| oos_mounted(&m))
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "/oos is not mounted (see /tmp/oos-data.log): the result would be lost at the next start",
        ));
    }
    std::fs::create_dir_all(dir)?;
    let path = format!("{}/{}", dir.trim_end_matches('/'), name);
    let tmp = format!("{}.new", path);
    let mut f = std::fs::File::create(&tmp)?;
    let result = f.write_all(&build(payload, s)).and_then(|()| f.sync_all());
    drop(f);
    if let Err(e) = result {
        remove_temporary(&tmp);
        return Err(e);
    }
    Ok(path)
}

/// The data partition is mounted at /oos, by the text of /proc/mounts.
pub fn oos_mounted(mounts: &str) -> bool {
    mounts.lines().any(|l| l.split_whitespace().nth(1) == Some("/oos"))
}

/// The file `name`: OOS's own copy if it is valid, else /rigol/data, else /rigol/data/default (stock order with the
/// OOS directory in front). Returns the bytes and the directory used.
pub fn load_first(name: &str, dirs: &[&str]) -> Option<(Vec<u8>, String)> {
    dirs.iter().find_map(|d| {
        let b = std::fs::read(format!("{}/{}", d.trim_end_matches('/'), name)).ok()?;
        parse(&b).ok()?;
        Some((b, d.to_string()))
    })
}

#[cfg(test)]
mod tests;
