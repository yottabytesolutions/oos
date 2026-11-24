// SPDX-License-Identifier: GPL-2.0-or-later

//! ADR 0011: the sole raw storage writer. FPGA flash, FRAM and misc have no writable range.
use super::{Card, BACKUP_LBA, BOOT_LBA, IMAGE_MAX, OOSDATA_LBA, RECOVERY_LBA, SECTOR};
use std::io::{Seek, SeekFrom, Write};

mod data;

/// Absolute byte ranges on the whole card. Device paths are never area names.
pub fn check_range(area: &str, offset: u64, len: usize) -> Result<(), String> {
    let (start, size) = match area {
        "boot" => (BOOT_LBA * SECTOR, 4),
        "recovery" => (RECOVERY_LBA * SECTOR, IMAGE_MAX as u64),
        "backup" => {
            (BACKUP_LBA * SECTOR, (OOSDATA_LBA + crate::target::ACTIVE.layout.oosdata_sectors - BACKUP_LBA) * SECTOR)
        }
        _ => return Err(format!("protected storage area: {area}")),
    };
    let end = offset.checked_add(len as u64).ok_or("storage range overflow")?;
    if len == 0 || offset < start || end > start + size {
        return Err(format!("write outside {area}: offset {offset:#x}, length {len}"));
    }
    Ok(())
}

/// Only this module owns a writable card descriptor.
#[derive(Debug)]
pub(super) struct Storage {
    file: std::fs::File,
    writable: bool,
}

impl Storage {
    pub(super) fn open(path: &std::path::Path, writable: bool, removable: bool) -> std::io::Result<Self> {
        let file = std::fs::OpenOptions::new().read(true).write(writable).open(path)?;
        super::path::check_file(&file, path, removable)?;
        Ok(Self { file, writable })
    }

    pub(super) fn writable(&self) -> bool {
        self.writable
    }

    pub(super) fn is_file(&self) -> std::io::Result<bool> {
        Ok(self.file.metadata()?.is_file())
    }

    pub(super) fn read(&mut self, offset: u64, len: usize) -> std::io::Result<Vec<u8>> {
        let mut bytes = vec![0; len];
        self.read_into(offset, &mut bytes)?;
        Ok(bytes)
    }

    pub(super) fn read_into(&mut self, offset: u64, bytes: &mut [u8]) -> std::io::Result<()> {
        self.file.seek(SeekFrom::Start(offset))?;
        std::io::Read::read_exact(&mut self.file, bytes)
    }

    fn write(
        &mut self,
        area: &str,
        offset: u64,
        data: &[u8],
        cut: bool,
        #[cfg(test)] fault: Option<super::WriteFault>,
    ) -> Result<(), String> {
        check_range(area, offset, data.len())?;
        if !self.writable {
            return Err("card opened read only".into());
        }
        let io = |e: std::io::Error| format!("writing {area}: {e}");
        self.file.seek(SeekFrom::Start(offset)).map_err(io)?;
        if cut {
            self.file.write_all(&data[..data.len() / 2]).map_err(io)?;
            return Err("power cut".into());
        }
        #[cfg(test)]
        if matches!(fault, Some(super::WriteFault::PowerCut)) {
            self.file.write_all(&[0]).map_err(io)?;
            self.file.write_all(&data[1..data.len().div_ceil(2)]).map_err(io)?;
            std::panic::panic_any("simulated power cut");
        }
        self.file.write_all(data).map_err(io)?;
        #[cfg(test)]
        if matches!(fault, Some(super::WriteFault::ReadBackMismatch)) {
            self.file.seek(SeekFrom::Start(offset)).map_err(io)?;
            self.file.write_all(&[!data[0]]).map_err(io)?;
        }
        self.file.sync_all().map_err(io)?;
        self.drop_cache(offset, data.len());
        if self.read(offset, data.len()).map_err(io)? != data {
            return Err(format!("READ-BACK MISMATCH in {area}"));
        }
        #[cfg(test)]
        if matches!(fault, Some(super::WriteFault::CutAfterReadBack)) {
            return Err("power cut after read-back".into());
        }
        Ok(())
    }

    fn drop_cache(&self, offset: u64, len: usize) {
        #[cfg(target_os = "linux")]
        {
            use std::os::fd::AsRawFd;
            let (start, end) = (offset & !4095, (offset + len as u64 + 4095) & !4095);
            // SAFETY: the live descriptor and whole-page range are valid advisory inputs.
            unsafe {
                libc::posix_fadvise(
                    self.file.as_raw_fd(),
                    start as i64,
                    (end - start) as i64,
                    libc::POSIX_FADV_DONTNEED,
                );
            }
        }
        #[cfg(not(target_os = "linux"))]
        let _ = (offset, len);
    }
}

impl Card {
    /// Write only a named, bounded ADR 0011 area, then sync and verify it from the opened card.
    pub fn write_checked(&mut self, area: &str, offset: u64, data: &[u8]) -> Result<(), String> {
        check_range(area, offset, data.len())?;
        self.check_layout()?;
        #[cfg(test)]
        let cut = self.cut.is_some_and(|r| r.lba() * SECTOR == offset);
        #[cfg(test)]
        if cut {
            self.cut = None;
        }
        #[cfg(not(test))]
        let cut = false;
        #[cfg(test)]
        let fault = self.write_fault_at.take_if(|(at, _)| *at == offset).map(|(_, fault)| fault).or_else(|| {
            self.write_fault.take_if(|(region, _)| region.lba() * SECTOR == offset).map(|(_, fault)| fault)
        });
        self.storage.write(
            area,
            offset,
            data,
            cut,
            #[cfg(test)]
            fault,
        )
    }
}

#[cfg(test)]
mod tests;
