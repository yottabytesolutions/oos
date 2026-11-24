// SPDX-License-Identifier: GPL-2.0-or-later

use super::{WriteKind, SECTOR};
use std::ops::Range;

pub(super) const PRIMARY: Range<usize> = 1024..2048;
const SUPER_SIZE: usize = 1024;
pub(super) const HAS_JOURNAL: u32 = 0x0004;
const NEEDS_RECOVERY: u32 = 0x0004;
const SECTOR_SIZE: usize = SECTOR as usize;
const META_BG: u32 = 0x10;
const DESC_64BIT: u32 = 0x80;
const SPARSE_SUPER: u32 = 0x01;
const SPARSE_SUPER2: u32 = 0x200;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Source {
    Empty,
    Primary,
    Backup { block: usize, block_size: usize },
}

pub(super) fn source(bytes: &[u8]) -> Option<Source> {
    if Superblock::read(bytes).is_some() {
        return Some(Source::Primary);
    }
    let (blocks, _) = bytes.as_chunks::<SUPER_SIZE>();
    for (index, raw) in blocks.iter().enumerate().skip(2) {
        let offset = index * SUPER_SIZE;
        if let Some((sb, geometry)) = Superblock::parse(raw, offset)
            .and_then(|sb| Geometry::parse(sb, bytes.len()).map(|geometry| (sb, geometry)))
        {
            let block = offset / geometry.block_size;
            if offset.is_multiple_of(geometry.block_size) && geometry.is_backup(block, sb.word(0x5a)) {
                return Some(Source::Backup { block, block_size: geometry.block_size });
            }
        }
    }
    bytes.iter().all(|&b| b == 0).then_some(Source::Empty)
}

#[derive(Clone, Copy)]
pub(super) struct Superblock<'a> {
    raw: &'a [u8; SUPER_SIZE],
    offset: usize,
}

impl<'a> Superblock<'a> {
    pub(super) fn read(image: &'a [u8]) -> Option<Self> {
        Self::read_at(image, PRIMARY.start)
    }

    pub(super) fn read_at(image: &'a [u8], offset: usize) -> Option<Self> {
        Self::parse(image.get(offset..)?.first_chunk()?, offset)
    }

    fn parse(raw: &'a [u8; SUPER_SIZE], offset: usize) -> Option<Self> {
        (raw[0x38..0x3a] == [0x53, 0xef]).then_some(Self { raw, offset })
    }

    pub(super) fn word(self, offset: usize) -> u16 {
        u16::from_le_bytes([self.raw[offset], self.raw[offset + 1]])
    }

    pub(super) fn dword(self, offset: usize) -> u32 {
        u32::from(self.word(offset)) | (u32::from(self.word(offset + 2)) << 16)
    }

    pub(super) fn block_size(self) -> Option<usize> {
        let size = 1usize.checked_shl(self.dword(0x18).checked_add(10)?)?;
        (size <= 65_536).then_some(size)
    }

    pub(super) fn features(self) -> [u32; 3] {
        [self.dword(0x5c), self.dword(0x60), self.dword(0x64)]
    }

    pub(super) fn has_journal(self) -> bool {
        self.features()[0] & HAS_JOURNAL != 0
    }

    pub(super) fn needs_recovery(self) -> bool {
        self.features()[1] & NEEDS_RECOVERY != 0
    }

    pub(super) fn descriptor_offset(self, block_size: usize) -> usize {
        (self.offset / block_size + 1) * block_size
    }
}

enum Copies {
    All,
    Sparse,
    Selected(usize, usize),
}

pub(super) struct Geometry {
    block_size: usize,
    first_block: usize,
    blocks_per_group: usize,
    groups: usize,
    descriptors_per_block: usize,
    first_meta_group: Option<usize>,
    copies: Copies,
}

impl Geometry {
    pub(super) fn parse(sb: Superblock<'_>, image_len: usize) -> Option<Self> {
        let block_size = sb.block_size()?;
        let first_block = sb.dword(0x14) as usize;
        let blocks = sb.dword(0x04) as usize;
        let blocks_per_group = sb.dword(0x20) as usize;
        let [compat, incompatible, ro_compat] = sb.features();
        if first_block > usize::from(block_size == SUPER_SIZE)
            || blocks <= first_block
            || blocks > image_len / block_size
            || blocks_per_group == 0
            || (incompatible & DESC_64BIT != 0 && sb.dword(0x150) != 0)
        {
            return None;
        }
        let desc_size = if incompatible & DESC_64BIT == 0 { 32 } else { usize::from(sb.word(0xfe)) };
        if !(32..=SUPER_SIZE).contains(&desc_size) || !desc_size.is_power_of_two() {
            return None;
        }
        let copies = if compat & SPARSE_SUPER2 != 0 {
            Copies::Selected(sb.dword(0x24c) as usize, sb.dword(0x250) as usize)
        } else if ro_compat & SPARSE_SUPER != 0 {
            Copies::Sparse
        } else {
            Copies::All
        };
        Some(Self {
            block_size,
            first_block,
            blocks_per_group,
            groups: (blocks - first_block).div_ceil(blocks_per_group),
            descriptors_per_block: block_size / desc_size,
            first_meta_group: (incompatible & META_BG != 0).then(|| sb.dword(0x104) as usize),
            copies,
        })
    }

    fn group_start(&self, group: usize) -> usize {
        self.first_block + group * self.blocks_per_group
    }

    // Called for sectors already classified as superblock or descriptor copies.
    pub(super) fn is_backup_descriptor(&self, offset: usize) -> bool {
        let block = offset / self.block_size;
        let relative = block.saturating_sub(self.first_block);
        relative >= self.blocks_per_group && !relative.is_multiple_of(self.blocks_per_group)
    }

    fn is_backup(&self, block: usize, stored_group: u16) -> bool {
        let Some(relative) = block.checked_sub(self.first_block) else { return false };
        let group = relative / self.blocks_per_group;
        // BusyBox copies the primary superblock verbatim, leaving its group number zero.
        relative.is_multiple_of(self.blocks_per_group)
            && group != 0
            && group < self.groups
            && (stored_group == 0 || usize::from(stored_group) == group)
            && self.has_super(group)
    }

    fn has_super(&self, group: usize) -> bool {
        group == 0
            || match self.copies {
                Copies::All => true,
                Copies::Sparse => [3, 5, 7].into_iter().any(|base| is_power(group, base)),
                Copies::Selected(first, second) => group == first || group == second,
            }
    }

    fn descriptor_blocks(&self, group: usize, has_super: bool) -> usize {
        let total = self.groups.div_ceil(self.descriptors_per_block);
        match self.first_meta_group {
            Some(first) if group / self.descriptors_per_block >= first => {
                let position = group % self.descriptors_per_block;
                usize::from(position <= 1 || position == self.descriptors_per_block - 1)
            }
            first => usize::from(has_super) * first.unwrap_or(total).min(total),
        }
    }
}

fn is_power(mut number: usize, base: usize) -> bool {
    while number.is_multiple_of(base) && number != 0 {
        number /= base;
    }
    number == 1
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Order {
    FormatCopies,
    Data,
    JournalInode,
    Copies,
    JournalHead,
    Intent,
    PrimaryTail,
    Primary,
}

impl Order {
    pub(super) const PASSES: [Self; 8] = [
        Self::FormatCopies,
        Self::Data,
        Self::JournalInode,
        Self::Copies,
        Self::JournalHead,
        Self::Intent,
        Self::PrimaryTail,
        Self::Primary,
    ];
}

/// Ext's group starts locate all copies, including descriptor-only groups with META_BG.
pub(super) fn write_order(bytes: &[u8], kind: WriteKind) -> Option<Vec<Order>> {
    let geometry = Geometry::parse(Superblock::read(bytes)?, bytes.len())?;
    let mut order = vec![Order::Data; bytes.len() / SECTOR_SIZE];
    let copies = if kind.is_format() { Order::FormatCopies } else { Order::Copies };
    if kind.is_format() {
        // Clear the intent while the backups still provide the only superblock copies.
        order[0] = Order::Intent;
    }
    for group in 0..geometry.groups {
        let superblock = geometry.has_super(group);
        let block = geometry.group_start(group).max(usize::from(geometry.block_size == SUPER_SIZE));
        let offset = block * geometry.block_size;
        if group != 0 && superblock {
            mark(&mut order, offset..offset + SUPER_SIZE, copies)?;
        }
        let descriptors = geometry.descriptor_blocks(group, superblock);
        if descriptors != 0 {
            let start = offset + usize::from(superblock) * geometry.block_size;
            mark(&mut order, start..start + descriptors * geometry.block_size, copies)?;
        }
    }
    // The sector with recovery and journal flags commits the entire write-back.
    let flags_end = PRIMARY.start + SECTOR_SIZE;
    mark(&mut order, flags_end..PRIMARY.end, Order::PrimaryTail)?;
    mark(&mut order, PRIMARY.start..flags_end, Order::Primary)?;
    if let Some((offset, priority)) = kind.journal() {
        let slot = order.get_mut(offset / SECTOR_SIZE).filter(|slot| **slot == Order::Data)?;
        *slot = priority;
    }
    Some(order)
}

fn mark(order: &mut [Order], bytes: Range<usize>, priority: Order) -> Option<()> {
    order.get_mut(bytes.start / SECTOR_SIZE..bytes.end / SECTOR_SIZE)?.fill(priority);
    Some(())
}
