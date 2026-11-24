// SPDX-License-Identifier: GPL-2.0-or-later

use super::geometry::{self, Geometry, Order, Source, Superblock, HAS_JOURNAL, PRIMARY};
use super::{equal_except, image_tool, verify_image, Image, WriteKind, SECTOR};
use std::ops::Range;

const SECTOR_BYTES: usize = SECTOR as usize;
const JOURNAL_INODE: u32 = 8;
const JOURNAL_BLOCKS: usize = 1024;
const MIN_AVAILABLE: usize = 1024 * 1024;

const JOURNAL_MAGIC: [u8; 4] = [0xc0, 0x3b, 0x39, 0x98];
const JOURNAL_LENGTH: Range<usize> = 16..20;
const JOURNAL_SEQUENCE: Range<usize> = 24..28;
const JOURNAL_START: Range<usize> = 28..32;

fn le_u32(bytes: &[u8]) -> Option<u32> {
    Some(u32::from_le_bytes(*bytes.first_chunk()?))
}

impl Superblock<'_> {
    fn classic_layout(self) -> bool {
        let [compat, incompat, ro_compat] = self.features();
        // ext_attr, resize_inode, dir_index; filetype; sparse_super, large_file.
        compat & !(0x0038 | HAS_JOURNAL) == 0 && incompat & !0x0006 == 0 && ro_compat & !0x0003 == 0
    }

    fn available_blocks(self) -> usize {
        self.dword(12).saturating_sub(self.dword(8)) as usize
    }

    pub(super) fn journal_inode(self, image: &[u8]) -> Option<usize> {
        if self.dword(224) != JOURNAL_INODE || !self.classic_layout() {
            return None;
        }
        let block_size = self.block_size()?;
        let inode_size = usize::from(self.word(88));
        if !matches!(inode_size, 128 | 256 | 512) {
            return None;
        }
        // The reserved journal inode is in group zero. Its descriptor follows this superblock copy.
        let descriptors = self.descriptor_offset(block_size);
        let table_block = le_u32(image.get(descriptors + 8..)?)? as usize;
        let offset = table_block.checked_mul(block_size)? + (JOURNAL_INODE as usize - 1) * inode_size;
        if table_block == 0 {
            return None;
        }
        image.get(offset..offset + inode_size)?;
        Some(offset)
    }

    pub(super) fn journal_sector(self, image: &[u8]) -> Option<usize> {
        let inode = self.journal_inode(image)?;
        // A classic inode's first direct pointer locates the JBD2 superblock even if its magic is corrupt.
        let block = le_u32(image.get(inode + 40..)?)? as usize;
        let offset = block.checked_mul(self.block_size()?)?;
        (block != 0 && image.get(offset..)?.len() >= SECTOR_BYTES).then_some(offset)
    }
}

#[derive(Clone, Copy)]
pub(super) struct Replay {
    head: usize,
    force: bool,
}

impl Replay {
    pub(super) fn head(self) -> usize {
        self.head
    }

    pub(super) fn args(self) -> [&'static str; 3] {
        // Older builds may have published a clean primary before writing replayed data.
        // Preen refuses to replay a nonempty journal if the recovery bit is clear.
        [if self.force { "-y" } else { "-p" }, "-E", "journal_only"]
    }

    pub(super) fn was_reset(self, before: &[u8], checked: &[u8]) -> bool {
        let Some(head) =
            Superblock::read(checked).filter(|sb| sb.has_journal()).and_then(|sb| sb.journal_sector(checked))
        else {
            return true;
        };
        let old = &before[self.head..];
        let new = &checked[head..];
        let word = |header: &[u8], field: Range<usize>| {
            let at = field.start;
            u32::from_be_bytes([header[at], header[at + 1], header[at + 2], header[at + 3]])
        };
        let advance = word(new, JOURNAL_SEQUENCE).wrapping_sub(word(old, JOURNAL_SEQUENCE));
        // Each replayed transaction needs a log block. Even with the final sequence
        // increment, replay cannot advance farther than the journal's block count.
        old[..JOURNAL_SEQUENCE.start] != new[..JOURNAL_SEQUENCE.start]
            || advance > word(old, JOURNAL_LENGTH)
            || (old[JOURNAL_START] != [0; 4] && advance == 0)
    }
}

pub(super) fn pending_replay(image: &[u8], source: Source) -> Result<Option<Replay>, String> {
    let offset = match source {
        Source::Empty => return Ok(None),
        Source::Primary => PRIMARY.start,
        Source::Backup { block, block_size } => block * block_size,
    };
    let sb = Superblock::read_at(image, offset).ok_or("missing OOS data superblock: nothing written")?;
    let needs_recovery = sb.needs_recovery();
    if !needs_recovery && !sb.has_journal() {
        return Ok(None);
    }
    let Some(head) = sb.journal_sector(image) else {
        // An early first-format cut can publish a backup before the journal inode exists.
        return if needs_recovery {
            Err("cannot order writes for this journal: nothing written".into())
        } else {
            Ok(None)
        };
    };
    let valid_magic = image[head..head + 4] == JOURNAL_MAGIC;
    let has_transactions = image[head..][JOURNAL_START] != [0; 4];
    let force = !needs_recovery && valid_magic && has_transactions;
    Ok((needs_recovery || force || !valid_magic).then_some(Replay { head, force }))
}

pub(super) fn conversion_inode(image: &[u8]) -> Result<usize, String> {
    let inode = Superblock::read(image).and_then(|sb| sb.journal_inode(image)).ok_or("missing journal inode")?;
    // Publish the journal's storage before publishing the inode that points to it.
    Ok(inode / SECTOR_BYTES * SECTOR_BYTES)
}

/// Identify a checked ext2 image that only needed to back out writes from the old conversion.
pub(super) fn was_conversion(before: &[u8], checked: &[u8], converted: &[u8], inode: usize) -> bool {
    const SUPER_TIMES: [Range<usize>; 2] = [48..52, 64..68]; // Last write and last check.
    let Some(old) = Superblock::read(before).filter(|sb| sb.classic_layout() && !sb.has_journal()) else {
        return false;
    };
    let Some(geometry) = Geometry::parse(old, before.len()) else { return false };
    let Some(order) = geometry::write_order(converted, WriteKind::Conversion(inode)) else { return false };
    let Some(journal) = Superblock::read(converted).and_then(|sb| sb.journal_inode(converted)) else { return false };
    let at = journal % SECTOR_BYTES;
    let mut observed = false;
    let sectors = before.as_chunks::<SECTOR_BYTES>().0.iter().zip(checked.as_chunks::<SECTOR_BYTES>().0);
    for (index, ((old, checked), converted)) in sectors.zip(converted.as_chunks::<SECTOR_BYTES>().0).enumerate() {
        if old == checked {
            continue;
        }
        let offset = index * SECTOR_BYTES;
        let matches_conversion = match order[index] {
            Order::Primary if equal_except(old, checked, &SUPER_TIMES) => continue,
            Order::JournalInode => {
                let creation_time = if at + 148 <= SECTOR_BYTES { at + 144..at + 148 } else { 0..0 };
                equal_except(old, converted, &[at + 8..at + 20, creation_time])
            }
            Order::Copies if geometry.is_backup_descriptor(offset) => {
                // fsck may refresh the stale accounting from the primary descriptor table.
                if equal_backup_descriptors(old, checked) {
                    continue;
                }
                old == converted
            }
            Order::Copies if Superblock::read_at(before, offset).is_some() => {
                equal_except(old, converted, &SUPER_TIMES)
            }
            _ => old == converted,
        };
        if !matches_conversion {
            return false;
        }
        observed |= matches!(order[index], Order::Data | Order::JournalInode);
    }
    observed
}

fn equal_backup_descriptors(old: &[u8; SECTOR_BYTES], new: &[u8; SECTOR_BYTES]) -> bool {
    old.as_chunks::<32>().0.iter().zip(new.as_chunks::<32>().0).all(|(old, new)| {
        // Block and inode free counts and the directory count in a classic group descriptor.
        old[..12] == new[..12] && old[18..] == new[18..]
    })
}

fn allocation_blocks(block_size: usize) -> usize {
    // A classic inode has 12 direct pointers, then a single- and a double-indirect tree.
    let pointers_per_block = block_size / 4;
    let double_indirect = (JOURNAL_BLOCKS - 12).saturating_sub(pointers_per_block);
    JOURNAL_BLOCKS + 1 + usize::from(double_indirect != 0) + double_indirect.div_ceil(pointers_per_block)
}

pub(super) fn convert(image: &Image, before: &[u8]) -> Result<Option<Vec<u8>>, String> {
    let old = Superblock::read(before).ok_or("checked OOS data has no ext superblock")?;
    if old.has_journal() {
        return Ok(None);
    }
    let Some(block_size) = old.block_size().filter(|_| old.classic_layout() && !old.needs_recovery()) else {
        eprintln!(
            "oos-data: keeping existing filesystem features; journal conversion requires the classic ext2 layout"
        );
        return Ok(None);
    };
    let min_blocks = MIN_AVAILABLE / block_size;
    if old.available_blocks() < allocation_blocks(block_size) + min_blocks {
        eprintln!("oos-data: keeping ext2; the journal would leave less than 1 MiB available after reserved blocks");
        return Ok(None);
    }
    let size = format!("size={}", JOURNAL_BLOCKS * block_size / (1024 * 1024));
    let status = image_tool("tune2fs", &["-j", "-J", &size], &image.0)?;
    if !status.success() {
        return Err(format!("creating the data journal failed ({status})"));
    }
    verify_image(&image.0)?;
    let after = image.read(before.len())?;
    let new = Superblock::read(&after).ok_or("journal conversion removed the ext superblock")?;
    let [compat, incompat, ro_compat] = old.features();
    if new.features() != [compat | HAS_JOURNAL, incompat, ro_compat]
        || !new.journal_sector(&after).is_some_and(|head| after[head..head + 4] == JOURNAL_MAGIC)
    {
        return Err("journal conversion changed unexpected ext features or did not create an internal journal".into());
    }
    if new.available_blocks() < min_blocks {
        eprintln!("oos-data: keeping ext2; the journal would leave less than 1 MiB available after reserved blocks");
        return Ok(None);
    }
    Ok(Some(after))
}
