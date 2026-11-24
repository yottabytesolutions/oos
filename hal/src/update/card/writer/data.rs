// SPDX-License-Identifier: GPL-2.0-or-later
//! Filesystem tools work on a regular image; only the checked writer touches the card.
use super::*;
use std::fs::{self, OpenOptions};
use std::ops::Range;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::process::{Command, ExitStatus};

mod geometry;
use geometry::{Order, Source};

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

fn check_image(path: &Path, source: Source) -> Result<(), String> {
    if source == Source::Empty {
        let status = image_tool("mke2fs", &["-F", "-L", "oosdata"], path)?;
        if !status.success() {
            return Err(format!("formatting OOS data failed ({status}): nothing written"));
        }
    }
    check_until_clean(source, |args| {
        image_tool("e2fsck", args, path).and_then(check_result).map_err(|e| format!("e2fsck {}: {e}", args.join(" ")))
    })
}

fn check_until_clean(source: Source, mut check: impl FnMut(&[&str]) -> Result<Check, String>) -> Result<(), String> {
    let repair = match source {
        Source::Backup { block, block_size } => {
            check(&["-fy", "-b", &block.to_string(), "-B", &block_size.to_string()])
        }
        _ => {
            if let Err(error) = check(&["-p"]) {
                eprintln!("{error}");
            }
            if check(&["-fn"]) == Ok(Check::Clean) {
                return Ok(());
            }
            check(&["-fy"])
        }
    };
    if let Err(error) = repair {
        eprintln!("{error}");
    }
    match check(&["-fn"]) {
        Ok(Check::Clean) => Ok(()),
        Ok(result) => Err(format!("OOS data did not pass the read-only check ({result:?}): nothing written")),
        Err(error) => Err(format!("OOS data did not pass the read-only check ({error}): nothing written")),
    }
}

struct Image(std::path::PathBuf);

impl Image {
    fn copy_from(bytes: &[u8]) -> std::io::Result<Self> {
        let pid = std::process::id();
        for slot in 0..16 {
            let path = std::env::temp_dir().join(format!("oos-data-{pid}-{slot}.img"));
            match OpenOptions::new().write(true).create_new(true).mode(0o600).open(&path) {
                Ok(mut file) => {
                    let image = Self(path);
                    file.write_all(bytes)?;
                    return Ok(image);
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e),
            }
        }
        Err(std::io::Error::other("no free OOS data scratch image slot"))
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
        self.repair_data(check_image)
    }

    fn repair_data(
        &mut self,
        repair: impl FnOnce(&Path, Source) -> Result<(), String>,
    ) -> Result<Option<String>, String> {
        self.check_layout()?;
        let len = (crate::target::ACTIVE.layout.oosdata_sectors * SECTOR) as usize;
        let before = self.read(OOSDATA_LBA, len).map_err(|e| e.to_string())?;
        let source = geometry::source(&before).ok_or("OOS data is neither ext nor empty: nothing written")?;
        let image = Image::copy_from(&before).map_err(|e| format!("copying OOS data for repair: {e}"))?;
        repair(&image.0, source)?;
        let after = fs::read(&image.0).map_err(|e| e.to_string())?;
        if after.len() != len {
            return Err("filesystem tool changed OOS data size: nothing written".into());
        }
        let changed = self.write_data_changes(&before, &after)?;
        if changed != 0 {
            eprintln!("OOS data: {changed} sectors rewritten");
        }
        Ok((source != Source::Empty && changed != 0).then(|| "The data area was repaired".into()))
    }

    fn write_data_changes(&mut self, before: &[u8], after: &[u8]) -> Result<usize, String> {
        if before == after {
            return Ok(0);
        }
        let mut changed = 0;
        for run in data_runs(before, after)? {
            self.write_checked("backup", OOSDATA_LBA * SECTOR + run.start as u64, &after[run.clone()])?;
            changed += run.len() / SECTOR as usize;
        }
        Ok(changed)
    }
}

fn data_runs(before: &[u8], after: &[u8]) -> Result<Vec<Range<usize>>, String> {
    let order = geometry::write_order(after).ok_or("invalid OOS data geometry after repair: nothing written")?;
    let sector = SECTOR as usize;
    let mut runs = Vec::new();
    for pass in [Order::Data, Order::Copies, Order::Primary] {
        let mut pending = None;
        let pairs = before.chunks_exact(sector).zip(after.chunks_exact(sector));
        let changed = pairs.zip(&order).map(|((old, new), &priority)| priority == pass && old != new);
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
