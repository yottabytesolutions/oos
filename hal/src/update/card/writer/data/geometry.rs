// SPDX-License-Identifier: GPL-2.0-or-later

use super::SECTOR;
use std::ops::Range;

pub(super) const PRIMARY: Range<usize> = 1024..2048;
const SUPER_SIZE: usize = 1024;
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
    if has_magic(bytes.get(PRIMARY.clone())?) {
        return Some(Source::Primary);
    }
    let (blocks, _) = bytes.as_chunks::<SUPER_SIZE>();
    for (index, sb) in blocks.iter().enumerate().skip(2) {
        if let Some(geometry) = Geometry::parse(sb, bytes.len()) {
            let offset = index * SUPER_SIZE;
            let block = offset / geometry.block_size;
            if offset.is_multiple_of(geometry.block_size) && geometry.is_backup(block, word(sb, 0x5a)) {
                return Some(Source::Backup { block, block_size: geometry.block_size });
            }
        }
    }
    bytes.iter().all(|&b| b == 0).then_some(Source::Empty)
}

fn has_magic(sb: &[u8]) -> bool {
    word(sb, 0x38) == 0xef53
}

fn word(sb: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([sb[offset], sb[offset + 1]])
}

fn dword(sb: &[u8], offset: usize) -> u32 {
    u32::from(word(sb, offset)) | (u32::from(word(sb, offset + 2)) << 16)
}

enum Copies {
    All,
    Sparse,
    Selected(usize, usize),
}

struct Geometry {
    block_size: usize,
    first_block: usize,
    blocks_per_group: usize,
    groups: usize,
    descriptors_per_block: usize,
    first_meta_group: Option<usize>,
    copies: Copies,
}

impl Geometry {
    fn parse(sb: &[u8], image_len: usize) -> Option<Self> {
        if !has_magic(sb) {
            return None;
        }
        let block_size = 1usize.checked_shl(dword(sb, 0x18).checked_add(10)?)?;
        let first_block = dword(sb, 0x14) as usize;
        let blocks = dword(sb, 0x04) as usize;
        let blocks_per_group = dword(sb, 0x20) as usize;
        let incompatible = dword(sb, 0x60);
        if !(SUPER_SIZE..=65_536).contains(&block_size)
            || first_block > usize::from(block_size == SUPER_SIZE)
            || blocks <= first_block
            || blocks > image_len / block_size
            || blocks_per_group == 0
            || (incompatible & DESC_64BIT != 0 && dword(sb, 0x150) != 0)
        {
            return None;
        }
        let desc_size = if incompatible & DESC_64BIT == 0 { 32 } else { usize::from(word(sb, 0xfe)) };
        if !(32..=SUPER_SIZE).contains(&desc_size) || !desc_size.is_power_of_two() {
            return None;
        }
        let copies = if dword(sb, 0x5c) & SPARSE_SUPER2 != 0 {
            Copies::Selected(dword(sb, 0x24c) as usize, dword(sb, 0x250) as usize)
        } else if dword(sb, 0x64) & SPARSE_SUPER != 0 {
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
            first_meta_group: (incompatible & META_BG != 0).then(|| dword(sb, 0x104) as usize),
            copies,
        })
    }

    fn group_start(&self, group: usize) -> usize {
        self.first_block + group * self.blocks_per_group
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
    Data,
    Copies,
    Primary,
}

/// Ext's group starts locate all copies, including descriptor-only groups with META_BG.
pub(super) fn write_order(bytes: &[u8]) -> Option<Vec<Order>> {
    let geometry = Geometry::parse(bytes.get(PRIMARY.clone())?, bytes.len())?;
    let mut order = vec![Order::Data; bytes.len() / SECTOR_SIZE];
    for group in 0..geometry.groups {
        let superblock = geometry.has_super(group);
        let block = geometry.group_start(group).max(usize::from(geometry.block_size == SUPER_SIZE));
        let offset = block * geometry.block_size;
        if group != 0 && superblock {
            mark(&mut order, offset..offset + SUPER_SIZE, Order::Copies)?;
        }
        let descriptors = geometry.descriptor_blocks(group, superblock);
        if descriptors != 0 {
            let start = offset + usize::from(superblock) * geometry.block_size;
            mark(&mut order, start..start + descriptors * geometry.block_size, Order::Copies)?;
        }
    }
    mark(&mut order, PRIMARY, Order::Primary)?;
    Some(order)
}

fn mark(order: &mut [Order], bytes: Range<usize>, priority: Order) -> Option<()> {
    order.get_mut(bytes.start / SECTOR_SIZE..bytes.end / SECTOR_SIZE)?.fill(priority);
    Some(())
}
