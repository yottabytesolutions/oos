// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! The boot card as the update sees it: the stock PARM layout, the two image regions and the boot magic. The numbers come
//! from the board description (`crate::target`); on the DHO800/DHO900:
//!
//! | LBA | sectors | region |
//! |---:|---:|---|
//! | 122880 | 1 | first sector of `boot`: magic `KRNL` (stock boots) or `OOSK` (the loader falls back to `recovery`) |
//! | 188416 | 0x20000 (64 MiB) | `recovery`: the running OOS image |
//! | 319488 | 0x10000 (32 MiB) | start of `backup`: copy of the stock `boot` image (the loader's last fallback), never written here |
//! | 385024 | 0x20000 (64 MiB) | `backup` + 32 MiB: **the previous OOS image** (rollback slot) |
//! | 516096 | 0x8000 (16 MiB) | end of `backup`: the OOS data partition /oos, formatted and repaired offline |
//!
//! Every raw write goes through the checked writer (ADR 0011). Images are padded to whole sectors; the boot
//! selector changes only four bytes. Writes sync, drop cached pages and verify the bytes from the opened card.

use super::hash::{hex, sha1, Sha256};
use crate::target::ACTIVE;
use std::path::Path;

mod path;
mod writer;
pub use path::{open_checked, open_removable};
pub use writer::check_range;

pub const SECTOR: u64 = 512;
pub const PARM_LBA: u64 = ACTIVE.layout.parm_lba;
pub const BOOT_LBA: u64 = ACTIVE.layout.boot_lba;
pub const RECOVERY_LBA: u64 = ACTIVE.layout.recovery_lba;
pub const BACKUP_LBA: u64 = ACTIVE.layout.backup_lba;
pub const SLOT_LBA: u64 = ACTIVE.layout.slot_lba;
pub const IMAGE_SECTORS: u64 = ACTIVE.layout.image_sectors;
pub const OOSDATA_LBA: u64 = ACTIVE.layout.oosdata_lba;
pub const IMAGE_MAX: usize = (IMAGE_SECTORS * SECTOR) as usize;

const _: () = assert!(SLOT_LBA + IMAGE_SECTORS == OOSDATA_LBA);
const _: () = assert!(RECOVERY_LBA + IMAGE_SECTORS <= BACKUP_LBA);

/// The places of the card OOS writes images to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Region {
    Recovery,
    Slot,
    /// start of `backup`: the stock copy the loader falls back to
    Fallback,
}

impl Region {
    pub fn lba(self) -> u64 {
        match self {
            Region::Recovery => RECOVERY_LBA,
            Region::Slot => SLOT_LBA,
            Region::Fallback => BACKUP_LBA,
        }
    }
    /// Size of the region in bytes.
    pub fn max(self) -> usize {
        match self {
            Region::Recovery | Region::Slot => IMAGE_MAX,
            Region::Fallback => ((SLOT_LBA - BACKUP_LBA) * SECTOR) as usize,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Region::Recovery => "recovery",
            Region::Slot => "rollback slot (backup + 32 MiB)",
            Region::Fallback => "stock copy (start of backup)",
        }
    }
}

/// What the header of an Android boot image (v0, as `boot/oos-bootimg.py` builds it) says.
#[derive(Clone, Debug, PartialEq)]
pub struct ImageInfo {
    /// Bytes up to the end of the last section, page-padded.
    pub len: usize,
    pub name: String,
    /// The loader's id (SHA-1, first 20 bytes of the id field).
    pub id: [u8; 20],
}

fn le32(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

/// Header fields only (first 2 KiB): length and name, no id check.
pub fn image_header(b: &[u8]) -> Result<ImageInfo, String> {
    if b.len() < 608 || &b[..8] != b"ANDROID!" {
        return Err("no Android boot image (magic ANDROID!)".into());
    }
    let (ks, rs, ss, page) = (le32(b, 8) as usize, le32(b, 16) as usize, le32(b, 24) as usize, le32(b, 36) as usize);
    if page != 2048 {
        return Err(format!("page size {} (the loader takes 2048)", page));
    }
    let up = |n: usize| n.div_ceil(page) * page;
    let len = page + up(ks) + up(rs) + up(ss);
    if len > IMAGE_MAX {
        return Err(format!("image of {} bytes does not fit recovery ({} bytes)", len, IMAGE_MAX));
    }
    let name = String::from_utf8_lossy(&b[48..64]).trim_end_matches('\0').to_string();
    let mut id = [0u8; 20];
    id.copy_from_slice(&b[576..596]);
    Ok(ImageInfo { len, name, id })
}

pub(super) fn loader_id(header: &[u8], kernel: &[u8], ramdisk: &[u8], second: &[u8]) -> [u8; 20] {
    sha1(&[
        kernel,
        &(kernel.len() as u32).to_le_bytes(),
        ramdisk,
        &(ramdisk.len() as u32).to_le_bytes(),
        second,
        &(second.len() as u32).to_le_bytes(),
        &header[32..576],
    ])
}

/// The whole image: header, section lengths and the loader's SHA-1 id (an image the loader would refuse is
/// refused here, before anything is written).
pub fn check_image(b: &[u8]) -> Result<ImageInfo, String> {
    let info = image_header(b)?;
    if b.len() < info.len {
        return Err(format!("image is {} bytes, its header needs {}", b.len(), info.len));
    }
    let (ks, rs, ss) = (le32(b, 8) as usize, le32(b, 16) as usize, le32(b, 24) as usize);
    for (n, s) in [("kernel", ks), ("ramdisk", rs), ("second", ss)] {
        if s % 4 != 0 {
            return Err(format!("{} length {} is not a multiple of 4 (the loader's hash engine refuses it)", n, s));
        }
    }
    let page = 2048;
    let k0 = page;
    let r0 = k0 + ks.div_ceil(page) * page;
    let s0 = r0 + rs.div_ceil(page) * page;
    let id = loader_id(b, &b[k0..k0 + ks], &b[r0..r0 + rs], &b[s0..s0 + ss]);
    if id != info.id || b[596..608].iter().any(|&x| x != 0) {
        return Err(format!(
            "image id mismatch (header {}, content {}): the loader would refuse it",
            hex(&info.id),
            hex(&id)
        ));
    }
    if ks == 0 || &b[k0 + 0x38..k0 + 0x3c] != b"ARM\x64" {
        return Err("kernel is not an arm64 Image".into());
    }
    // where the loader puts the parts (docs/boot.md): the kernel at 0x280000 below 0x3a00000, ramdisk and second at
    // 0x4bf0000 together below 0x3000000 bytes
    let (kaddr, raddr) = (le32(b, 12), le32(b, 20));
    if kaddr != 0x0028_0000 || raddr != 0x04bf_0000 {
        return Err(format!(
            "load addresses kernel {:#x}, ramdisk {:#x} (the loader uses 0x280000 and 0x4bf0000)",
            kaddr, raddr
        ));
    }
    if 0x0028_0000 + ks > 0x03a0_0000 {
        return Err(format!("kernel of {} bytes reaches past 0x3a00000", ks));
    }
    if rs + ss >= 0x0300_0000 {
        return Err(format!("ramdisk and second of {} bytes reach past 0x3000000", rs + ss));
    }
    Ok(info)
}

#[cfg(test)]
pub(super) enum WriteFault {
    PowerCut,
    CutAfterReadBack,
    ReadBackMismatch,
}

/// The card (or, in tests, a file laid out like it).
pub struct Card {
    storage: writer::Storage,
    /// tests: the next write of this region stops half way, as in a power cut
    #[cfg(test)]
    pub cut: Option<Region>,
    #[cfg(test)]
    pub(super) write_fault: Option<(Region, WriteFault)>,
    #[cfg(test)]
    write_fault_at: Option<(u64, WriteFault)>,
    #[cfg(test)]
    pub(super) read_bytes: usize,
}

impl Card {
    /// Only an opened regular file can opt into simulated units and direct test installs.
    pub fn is_test_file(&self) -> std::io::Result<bool> {
        self.storage.is_file()
    }

    pub fn open(path: &Path, write: bool) -> std::io::Result<Card> {
        Self::open_at(path, write, false)
    }

    fn open_at(path: &Path, write: bool, removable: bool) -> std::io::Result<Card> {
        Ok(Card {
            storage: writer::Storage::open(path, write, removable)?,
            #[cfg(test)]
            cut: None,
            #[cfg(test)]
            write_fault: None,
            #[cfg(test)]
            write_fault_at: None,
            #[cfg(test)]
            read_bytes: 0,
        })
    }

    pub fn read(&mut self, lba: u64, len: usize) -> std::io::Result<Vec<u8>> {
        #[cfg(test)]
        {
            self.read_bytes += len;
        }
        self.storage.read(lba * SECTOR, len)
    }

    pub(super) fn read_region_into(&mut self, region: Region, bytes: &mut [u8]) -> std::io::Result<()> {
        if bytes.len() > region.max() {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "read exceeds the card region"));
        }
        #[cfg(test)]
        {
            self.read_bytes += bytes.len();
        }
        self.storage.read_into(region.lba() * SECTOR, bytes)
    }

    /// The stock partition table (as `oos-oneshot` and `card-install.py` check it).
    pub fn check_layout(&mut self) -> Result<(), String> {
        let raw = self.read(PARM_LBA, 1024).map_err(|e| format!("cannot read the PARM block: {}", e))?;
        let text: String = raw.iter().filter(|&&c| c != 0).map(|&c| (c as char).to_ascii_lowercase()).collect();
        for w in ACTIVE.parm_marks {
            if !text.contains(w) {
                return Err(format!(
                    "unexpected partition table (no '{}'): not the stock {} layout, nothing written",
                    w, ACTIVE.family_name
                ));
            }
        }
        Ok(())
    }

    /// Header info of the image in `r`, if it holds one.
    pub fn header(&mut self, r: Region) -> std::io::Result<Option<ImageInfo>> {
        let h = self.read(r.lba(), 2048)?;
        Ok(image_header(&h).ok())
    }

    /// The image in `r`, `len` bytes, and its SHA-256.
    pub fn read_image(&mut self, r: Region, len: usize) -> std::io::Result<(Vec<u8>, [u8; 32])> {
        let b = self.read(r.lba(), len.min(IMAGE_MAX))?;
        let mut s = Sha256::new();
        s.update(&b);
        Ok((b, s.finish()))
    }

    /// True when the region holds only zeros (read in 1 MiB pieces).
    pub fn region_empty(&mut self, r: Region) -> std::io::Result<bool> {
        let step = 1 << 20;
        let mut off = 0usize;
        while off < r.max() {
            let b = self.read(r.lba() + (off as u64) / SECTOR, step)?;
            if b.iter().any(|&x| x != 0) {
                return Ok(false);
            }
            off += step;
        }
        Ok(true)
    }

    /// Write `img` to `r` and read it back from the card. Only these regions can be written, and only an image no
    /// longer than the region.
    pub fn write_image(&mut self, r: Region, img: &[u8]) -> Result<(), String> {
        if !self.storage.writable() {
            return Err("card opened read only".into());
        }
        if img.len() > r.max() {
            return Err(format!("{} bytes do not fit {}", img.len(), r.name()));
        }
        let off = r.lba() * SECTOR;
        let area = if r == Region::Recovery { "recovery" } else { "backup" };
        if img.len().is_multiple_of(SECTOR as usize) {
            return self.write_checked(area, off, img);
        }
        let mut data = img.to_vec();
        data.resize(img.len().div_ceil(SECTOR as usize) * SECTOR as usize, 0);
        self.write_checked(area, off, &data)
    }

    pub fn boot_magic(&mut self) -> std::io::Result<[u8; 4]> {
        let b = self.read(BOOT_LBA, 4)?;
        Ok([b[0], b[1], b[2], b[3]])
    }

    /// Select OOS for the next power-on (`KRNL` -> `OOSK` at the start of `boot`, like `oos-boot-oos-again`).
    /// Ok(false) when it already was `OOSK`.
    pub fn arm_oos(&mut self) -> Result<bool, String> {
        self.set_magic(b"OOSK", b"KRNL")
    }

    /// Select stock for the next power-on (`OOSK` -> `KRNL`, like `oos-oneshot`). Ok(false) when it already was `KRNL`.
    /// Before `recovery` is written: a power cut in the write then starts the intact stock image in `boot`. With `OOSK`
    /// the loader would refuse the half-written `recovery` and go on to `backup`, which may be empty (Rockusb).
    pub fn disarm_oos(&mut self) -> Result<bool, String> {
        self.set_magic(b"KRNL", b"OOSK")
    }

    fn set_magic(&mut self, to: &[u8; 4], from: &[u8; 4]) -> Result<bool, String> {
        let s = self.boot_magic().map_err(|e| e.to_string())?;
        if &s[..4] == to {
            return Ok(false);
        }
        if &s[..4] != from {
            return Err(format!(
                "boot magic is '{}', neither KRNL nor OOSK: left alone",
                String::from_utf8_lossy(&s[..4])
            ));
        }
        if !self.storage.writable() {
            return Err("card opened read only".into());
        }
        self.write_checked("boot", BOOT_LBA * SECTOR, to)?;
        Ok(true)
    }
}
