// SPDX-License-Identifier: GPL-2.0-or-later

use super::journal_cases::{format_journalled, record_sectors, test_prefixes};
use super::*;

const SECTOR_BYTES: usize = SECTOR as usize;

#[test]
fn first_format_completes_after_every_sector_cut() {
    require_tools();
    let mut fixture = Fixture::new("first-format-cuts");
    let (result, before, after) = fixture.run();
    assert_eq!(result, Ok(None));
    let mut writes = Vec::new();
    let format = WriteKind::Format(journal::conversion_inode(&after).unwrap());
    write_changes(&before, &after, format, record_sectors(&mut writes)).unwrap();
    let mut backup = after.clone();
    backup[geometry::PRIMARY.clone()].fill(0);
    let Some(Source::Backup { block, block_size }) = geometry::source(&backup) else {
        panic!("first format has no backup")
    };
    assert_eq!(writes.first().unwrap(), &(0, FORMAT_INTENT));
    assert_eq!(writes[1].0, geometry::PRIMARY.end);
    assert_eq!(writes[2].0, block * block_size);
    assert_eq!(writes[writes.len() - 3], (0, EMPTY_SECTOR));
    assert_eq!(writes.last().unwrap().0, geometry::PRIMARY.start);
    eprintln!("first format: checking all {} sector prefixes", writes.len() + 1);
    test_prefixes(&mut fixture, &before, &after, &writes, |fixture, notice| {
        assert!(matches!(notice, None | Some("The data area was repaired. Damaged entries may be missing. Check your saved files and look in lost+found for recovered copies.")));
        assert_journalled_without_marker(fixture);
    });
}

fn assert_journalled_without_marker(fixture: &Fixture) {
    let recovered = fs::read(&fixture.image.0).unwrap();
    assert!(Superblock::read(&recovered).unwrap().has_journal());
    assert!(recovered.starts_with(&EMPTY_SECTOR));
}

#[test]
fn repeated_first_format_recovers_from_a_cut_at_its_first_backup() {
    require_tools();
    let mut fixture = Fixture::new("repeated-first-format");
    let (result, blank, formatted) = fixture.run();
    assert_eq!(result, Ok(None));
    let mut writes = Vec::new();
    let kind = WriteKind::Format(journal::conversion_inode(&formatted).unwrap());
    write_changes(&blank, &formatted, kind, record_sectors(&mut writes)).unwrap();

    let mut partial = blank;
    for (offset, sector) in writes.iter().take(2) {
        partial[*offset..][..SECTOR_BYTES].copy_from_slice(sector);
    }
    assert!(partial.starts_with(&FORMAT_INTENT));
    assert_eq!(geometry::source(&partial), None);
    fs::write(&fixture.image.0, &partial).unwrap();
    let (result, _, restarted) = fixture.run();
    assert_eq!(result, Ok(None));
    writes.clear();
    let kind = WriteKind::Format(journal::conversion_inode(&restarted).unwrap());
    write_changes(&partial, &restarted, kind, record_sectors(&mut writes)).unwrap();
    for (offset, sector) in writes {
        partial[offset..][..SECTOR_BYTES].copy_from_slice(&sector);
        if matches!(geometry::source(&partial), Some(Source::Backup { .. })) {
            fs::write(&fixture.image.0, &partial).unwrap();
            assert_repaired(&mut fixture);
            assert_journalled_without_marker(&fixture);
            return;
        }
    }
    panic!("repeated format never published a backup before the primary");
}

#[test]
fn stale_format_marker_preserves_files_with_a_primary_or_backup() {
    require_tools();
    let mut fixture = Fixture::new("stale-format-marker");
    format_journalled(&mut fixture);
    fixture.put_file("/calibration", b"synthetic calibration");
    fixture.put_file("/settings", b"synthetic settings");
    let paths = ["/calibration", "/settings"];
    let hashes = fixture.file_hashes(&paths);
    let mut original = fs::read(&fixture.image.0).unwrap();
    original[..SECTOR_BYTES].copy_from_slice(&FORMAT_INTENT);
    for torn_primary in [false, true] {
        if torn_primary {
            original[geometry::PRIMARY.start..][..SECTOR_BYTES].fill(0);
            assert!(matches!(geometry::source(&original), Some(Source::Backup { .. })));
        }
        fs::write(&fixture.image.0, &original).unwrap();
        let (notice, _, after) = fixture.run();
        assert!(after.starts_with(&EMPTY_SECTOR));
        fs::write(&fixture.image.0, &after).unwrap();
        assert_eq!(fixture.file_hashes(&paths), hashes);
        if torn_primary {
            assert_eq!(notice.unwrap().as_deref(), Some("The data area was repaired. Damaged entries may be missing. Check your saved files and look in lost+found for recovered copies."));
        } else {
            assert_eq!(notice, Ok(None));
            assert_eq!(after[SECTOR_BYTES..], original[SECTOR_BYTES..]);
        }
        assert_eq!(fixture.fsck("-fn"), 0);
    }
}
