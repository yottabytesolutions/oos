// SPDX-License-Identifier: GPL-2.0-or-later
//! Filesystem tools work on a regular image; only the checked writer touches the card.
use super::*;
use std::fs::{self, OpenOptions};
use std::ops::Range;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::process::{Command, ExitStatus};

mod geometry;
mod journal;
use geometry::{Order, Source, Superblock};
use journal::Replay;

const EMPTY_SECTOR: [u8; SECTOR as usize] = [0; SECTOR as usize];
const FORMAT_INTENT: [u8; SECTOR as usize] = {
    let id = b"OOSDATA:first-format:v1";
    let mut sector = EMPTY_SECTOR;
    sector.split_at_mut(id.len()).0.copy_from_slice(id);
    sector
};

const CORRECTED: i32 = 1 | 2;
const UNCORRECTED: i32 = 4;
const FAILURE: i32 = 8 | 16 | 32 | 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Check {
    Clean,
    Corrected,
    Uncorrected { corrected: bool },
}

// e2fsck(8) uses independent exit bits. Its reboot bit also means corrected;
// the image is offline, so rereading a mounted filesystem is unnecessary.
fn check_result(status: ExitStatus) -> Result<Check, String> {
    let bits = status.code().ok_or_else(|| format!("interrupted ({status})"))?;
    if bits & FAILURE != 0 || bits & !(CORRECTED | UNCORRECTED | FAILURE) != 0 {
        let reasons = [(8, "operational error"), (16, "usage error"), (32, "cancelled"), (128, "library error")];
        let mut reasons: Vec<_> =
            reasons.iter().filter_map(|&(bit, reason)| (bits & bit != 0).then_some(reason)).collect();
        if bits & !(CORRECTED | UNCORRECTED | FAILURE) != 0 {
            reasons.push("unknown exit bits");
        }
        return Err(format!("{} (exit {bits})", reasons.join(", ")));
    }
    let corrected = bits & CORRECTED != 0;
    Ok(if bits & UNCORRECTED != 0 {
        Check::Uncorrected { corrected }
    } else if corrected {
        Check::Corrected
    } else {
        Check::Clean
    })
}

fn image_tool(tool: &str, args: &[&str], path: &Path) -> Result<ExitStatus, String> {
    let output =
        Command::new(tool).args(args).arg(path).env("LC_ALL", "C").output().map_err(|e| format!("{tool}: {e}"))?;
    // Keep stdout reserved for the startup notice. The fsck transcript goes to the startup log.
    let mut log = std::io::stderr().lock();
    let _ = log.write_all(&output.stdout);
    let _ = log.write_all(&output.stderr);
    Ok(output.status)
}

fn check_image(path: &Path, source: Source, replay: Option<Replay>) -> Result<bool, String> {
    if source == Source::Empty {
        let status = image_tool("mke2fs", &["-F", "-b", "1024", "-L", "oosdata"], path)?;
        if !status.success() {
            return Err(format!("formatting OOS data failed ({status}): nothing written"));
        }
    }
    check_until_clean(source, replay, |args| run_check(path, args))
}

fn run_check(path: &Path, args: &[&str]) -> Result<Check, String> {
    image_tool("e2fsck", args, path).and_then(check_result).map_err(|e| format!("e2fsck {}: {e}", args.join(" ")))
}

fn check_until_clean(
    source: Source,
    replay: Option<Replay>,
    mut check: impl FnMut(&[&str]) -> Result<Check, String>,
) -> Result<bool, String> {
    let repair = match source {
        Source::Backup { block, block_size } => {
            // e2fsck replays an active journal before checking with the backup, even if its recovery flag is clear.
            check(&["-fy", "-b", &block.to_string(), "-B", &block_size.to_string()])
        }
        _ => {
            let journal = match replay {
                Some(replay) => check(&replay.args())?,
                None => Check::Clean,
            };
            if !matches!(journal, Check::Uncorrected { .. }) {
                let preen = check(&["-p"]);
                if let Err(error) = &preen {
                    eprintln!("{error}");
                }
                if check(&["-fn"]) == Ok(Check::Clean) {
                    return Ok(journal == Check::Corrected || preen == Ok(Check::Corrected));
                }
            }
            check(&["-fy"])
        }
    };
    if let Err(error) = repair {
        eprintln!("{error}");
    }
    require_clean(check(&["-fn"]))?;
    Ok(true)
}

fn require_clean(result: Result<Check, String>) -> Result<(), String> {
    match result {
        Ok(Check::Clean) => Ok(()),
        Ok(result) => Err(format!("OOS data did not pass the read-only check ({result:?}): nothing written")),
        Err(error) => Err(format!("OOS data did not pass the read-only check ({error}): nothing written")),
    }
}

fn verify_image(path: &Path) -> Result<(), String> {
    require_clean(run_check(path, &["-fn"]))
}

struct Image(std::path::PathBuf);

impl Image {
    fn copy_from(bytes: &[u8]) -> std::io::Result<Self> {
        Self::create(|mut file| file.write_all(bytes))
    }

    fn blank(len: usize) -> std::io::Result<Self> {
        Self::create(|file| file.set_len(len as u64))
    }

    fn create(initialize: impl FnOnce(fs::File) -> std::io::Result<()>) -> std::io::Result<Self> {
        let pid = std::process::id();
        for slot in 0_u64..=u64::MAX {
            let path = std::env::temp_dir().join(format!("oos-data-{pid}-{slot}.img"));
            match OpenOptions::new().write(true).create_new(true).mode(0o600).open(&path) {
                Ok(file) => {
                    let image = Self(path);
                    initialize(file)?;
                    return Ok(image);
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e),
            }
        }
        Err(std::io::Error::other("exhausted OOS data scratch image counter"))
    }

    fn read(&self, expected_len: usize) -> Result<Vec<u8>, String> {
        let bytes = fs::read(&self.0).map_err(|e| e.to_string())?;
        if bytes.len() != expected_len {
            return Err("filesystem tool changed OOS data size: nothing written".into());
        }
        Ok(bytes)
    }
}

impl Drop for Image {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

impl Card {
    /// Prepare the offline OOS data area for mounting and return a panel notice after a repair.
    pub fn prepare_data(&mut self) -> Result<Option<String>, String> {
        let (before, image, source) = self.stage_data()?;
        let replay = journal::pending_replay(&before, source)?;
        let repaired = check_image(&image.0, source, replay)?;
        let checked = image.read(before.len())?;
        if source == Source::Empty {
            let formatted = journal::convert(&image, &checked)?.ok_or("new OOS data must have a journal")?;
            self.write_data_changes(&before, &formatted, WriteKind::Format(journal::conversion_inode(&formatted)?))?;
            return Ok(None);
        }
        if journal::pending_replay(&checked, Source::Primary)?.is_some() {
            return Err("OOS data journal is still pending after the offline check: nothing written".into());
        }
        if before.starts_with(&FORMAT_INTENT) {
            self.write_data_sector(0, &EMPTY_SECTOR)?;
        }
        if !repaired && replay.is_none() && only_scratch_write_accounting(&before, &checked) {
            eprintln!("OOS data: 0 sectors rewritten (skipped scratch-image write accounting)");
            return Ok(None);
        }

        // Persist any repair before starting the independently retryable ext2 conversion.
        let changed =
            self.write_data_changes(&before, &checked, replay.map_or(WriteKind::Repair, WriteKind::Replay))?;
        if changed != 0 {
            eprintln!("OOS data: {changed} sectors rewritten");
        }
        let resumed_conversion = self.convert_data(&image, &before, &checked, changed != 0)?;
        let notice = if replay.is_some_and(|replay| replay.was_reset(&before, &checked)) {
            Some("The data area was repaired. Recent changes may be lost because the journal was reset. Check your settings and restore missing files from backup.")
        } else if repaired && !resumed_conversion {
            Some("The data area was repaired. Damaged entries may be missing. Check your saved files and look in lost+found for recovered copies.")
        } else {
            None
        };
        Ok(notice.filter(|_| changed != 0).map(str::to_owned))
    }

    fn convert_data(&mut self, image: &Image, before: &[u8], checked: &[u8], rewritten: bool) -> Result<bool, String> {
        match journal::convert(image, checked) {
            Ok(Some(converted)) => {
                let inode = journal::conversion_inode(&converted)?;
                let resumed = rewritten && journal::was_conversion(before, checked, &converted, inode);
                if resumed {
                    eprintln!("OOS data: resuming interrupted journal conversion");
                }
                self.write_data_changes(checked, &converted, WriteKind::Conversion(inode))?;
                Ok(resumed)
            }
            Ok(None) => Ok(false),
            Err(error) => {
                eprintln!("oos-data: keeping checked ext2; {error}");
                Ok(false)
            }
        }
    }

    /// Read the journal feature from the OOS data area's primary superblock.
    pub fn data_has_journal(&mut self) -> Result<bool, String> {
        self.check_layout()?;
        let header = self.read(OOSDATA_LBA, geometry::PRIMARY.end).map_err(|e| e.to_string())?;
        Superblock::read(&header).map(Superblock::has_journal).ok_or_else(|| "no ext superblock".into())
    }

    fn stage_data(&mut self) -> Result<(Vec<u8>, Image, Source), String> {
        self.check_layout()?;
        let len = (crate::target::ACTIVE.layout.oosdata_sectors * SECTOR) as usize;
        let before = self.read(OOSDATA_LBA, len).map_err(|e| e.to_string())?;
        let source = geometry::source(&before);
        let source = source
            .or_else(|| before.starts_with(&FORMAT_INTENT).then_some(Source::Empty))
            .ok_or("OOS data is neither ext nor empty: nothing written")?;
        let image = if source == Source::Empty { Image::blank(len) } else { Image::copy_from(&before) }
            .map_err(|e| format!("copying OOS data for repair: {e}"))?;
        Ok((before, image, source))
    }

    fn write_data_sector(&mut self, offset: usize, bytes: &[u8]) -> Result<(), String> {
        self.write_checked("backup", OOSDATA_LBA * SECTOR + offset as u64, bytes)
    }

    fn write_data_changes(&mut self, before: &[u8], after: &[u8], kind: WriteKind) -> Result<usize, String> {
        write_changes(before, after, kind, |offset, bytes| self.write_data_sector(offset, bytes))
    }
}

fn only_scratch_write_accounting(before: &[u8], checked: &[u8]) -> bool {
    // Preen can write an unchanged journal header on the scratch image. libext2fs
    // then updates s_wtime and s_kbytes_written even though the card had no writes.
    const WRITE_ACCOUNTING: [Range<usize>; 2] = [0x30..0x34, 0x178..0x180];
    let start = geometry::PRIMARY.start;
    let end = start + SECTOR as usize;
    let (old, new) = (&before[start..end], &checked[start..end]);
    old != new
        && Superblock::read(checked).is_some_and(Superblock::has_journal)
        && equal_except(old, new, &WRITE_ACCOUNTING)
        && before[..start] == checked[..start]
        && before[end..] == checked[end..]
}

fn equal_except(old: &[u8], new: &[u8], ignored: &[Range<usize>]) -> bool {
    old.iter().zip(new).enumerate().all(|(index, (a, b))| a == b || ignored.iter().any(|r| r.contains(&index)))
}

#[derive(Clone, Copy)]
enum WriteKind {
    Repair,
    Replay(Replay),
    Conversion(usize),
    Format(usize),
}

impl WriteKind {
    fn is_format(self) -> bool {
        matches!(self, Self::Format(_))
    }

    fn journal(self) -> Option<(usize, Order)> {
        match self {
            Self::Replay(replay) => Some((replay.head(), Order::JournalHead)),
            Self::Conversion(inode) | Self::Format(inode) => Some((inode, Order::JournalInode)),
            Self::Repair => None,
        }
    }
}

fn write_changes(
    before: &[u8],
    after: &[u8],
    kind: WriteKind,
    mut write: impl FnMut(usize, &[u8]) -> Result<(), String>,
) -> Result<usize, String> {
    let runs = data_runs(before, after, kind)?;
    if kind.is_format() {
        if !after.starts_with(&EMPTY_SECTOR) {
            return Err("formatter changed the OOS data intent sector: nothing written".into());
        }
        if !before.starts_with(&FORMAT_INTENT) {
            write(0, &FORMAT_INTENT)?;
        }
    }
    let mut changed = 0;
    for run in runs {
        write(run.start, &after[run.clone()])?;
        changed += run.len() / SECTOR as usize;
    }
    Ok(changed)
}

fn data_runs(before: &[u8], after: &[u8], kind: WriteKind) -> Result<Vec<Range<usize>>, String> {
    let sector = SECTOR as usize;
    if before.len() != after.len() || !after.len().is_multiple_of(sector) {
        return Err("invalid OOS data image sizes: nothing written".into());
    }
    let superblock = Superblock::read(after).ok_or("missing OOS data superblock after repair: nothing written")?;
    if superblock.needs_recovery() {
        return Err("OOS data still needs journal recovery: nothing written".into());
    }
    if before == after {
        return Ok(Vec::new());
    }
    let order = geometry::write_order(after, kind).ok_or("invalid OOS data write layout: nothing written")?;
    let mut runs = Vec::new();
    for pass in Order::PASSES {
        let mut pending = None;
        let pairs = before.chunks_exact(sector).zip(after.chunks_exact(sector));
        let changed =
            pairs.zip(&order).map(|((old, new), &priority)| priority == pass && (old != new || pass == Order::Intent));
        for (index, changed) in changed.chain([false]).enumerate() {
            if changed {
                pending.get_or_insert(index * sector);
            } else if let Some(start) = pending.take() {
                runs.push(start..index * sector);
            }
        }
    }
    Ok(runs)
}

#[cfg(test)]
mod tests;
