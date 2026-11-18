// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;

#[test]
fn all_allowed_areas_accept_their_exact_bounds() {
    let backup_end = (OOSDATA_LBA + crate::target::ACTIVE.layout.oosdata_sectors) * SECTOR;
    for (area, start, size) in [
        ("boot", BOOT_LBA * SECTOR, 4),
        ("recovery", RECOVERY_LBA * SECTOR, IMAGE_MAX as u64),
        ("backup", BACKUP_LBA * SECTOR, backup_end - BACKUP_LBA * SECTOR),
    ] {
        assert!(check_range(area, start, size as usize).is_ok());
        assert!(check_range(area, start + size - 1, 1).is_ok());
        for (offset, len) in [(start - 1, 2), (start, size as usize + 1), (start + size, 1), (u64::MAX, 2), (start, 0)]
        {
            assert!(check_range(area, offset, len).is_err(), "{area}: {offset} + {len}");
        }
    }
}

#[test]
fn protected_names_paths_and_flash_ranges_have_no_exception() {
    for area in [
        "p4",
        "p5",
        "uboot",
        "trust",
        "misc",
        "bcb",
        "resource",
        "kernel",
        "userdata",
        "vendor",
        "idb",
        "parameter",
        "fram",
        "fpga",
        "mtd0",
        "mtdblock0",
        "mmcblk0p4",
        "mmcblk0p5",
        "block/mmcblk0p4",
        "block/mmcblk0p5",
        "spidev1.0",
        "card",
    ] {
        for target in [area.to_string(), format!("/dev/{area}"), format!("/dev/block/by-name/{area}")] {
            // Even offsets inside allowed card areas cannot turn a protected target into an allowed one.
            for offset in
                [0, 0x400000, 0x7fffff, 0x800000, BOOT_LBA * SECTOR, RECOVERY_LBA * SECTOR, BACKUP_LBA * SECTOR]
            {
                assert!(check_range(&target, offset, 1).is_err(), "{target}: {offset:#x}");
            }
        }
    }
    for (offset, len) in [(0, 0x1000000), (0x3fffff, 2), (0x400000, 0x400000), (0x7fffff, 2), (0x800000, 0x400000)] {
        assert!(check_range("fpga", offset, len).is_err());
        for area in ["boot", "recovery", "backup"] {
            assert!(check_range(area, offset, len).is_err());
        }
    }
}
