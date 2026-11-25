// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::update::hash::sha256;

const SECTOR_BYTES: usize = SECTOR as usize;
const BLOCK_BYTES: usize = 1024;

pub(super) fn format_journalled(fixture: &mut Fixture) {
    fixture.format_ext2("1024", "8192");
    let (result, _, formatted) = fixture.run();
    assert_eq!(result, Ok(None));
    assert!(Superblock::read(&formatted).unwrap().has_journal());
    fs::write(&fixture.image.0, formatted).unwrap();
}

fn contents_hash(fixture: &Fixture, name: &str) -> [u8; 32] {
    sha256(&fixture.debugfs_bytes(&[], &format!("cat {name}")))
}

fn available(bytes: &[u8]) -> u32 {
    let blocks = |offset| u32::from_le_bytes(bytes[geometry::PRIMARY.start + offset..][..4].try_into().unwrap());
    blocks(12usize) - blocks(8usize)
}

type SectorWrite = (usize, [u8; SECTOR_BYTES]);

pub(super) fn record_sectors(writes: &mut Vec<SectorWrite>) -> impl FnMut(usize, &[u8]) -> Result<(), String> + '_ {
    |offset, bytes| {
        let (sectors, remainder) = bytes.as_chunks::<SECTOR_BYTES>();
        assert!(remainder.is_empty());
        writes.extend(sectors.iter().enumerate().map(|(index, &sector)| (offset + index * SECTOR_BYTES, sector)));
        Ok(())
    }
}

pub(super) fn test_prefixes(
    fixture: &mut Fixture,
    before: &[u8],
    after: &[u8],
    writes: &[SectorWrite],
    assert_recovered: impl Fn(&Fixture, Option<&str>),
) {
    let mut cut = before.to_vec();
    for prefix in 0..=writes.len() {
        fs::write(&fixture.image.0, &cut).unwrap();
        let (result, _, recovered) = fixture.run();
        let notice = result.unwrap_or_else(|error| panic!("cut after {prefix} sectors: {error}"));
        fs::write(&fixture.image.0, recovered).unwrap();
        assert_eq!(fixture.fsck("-fn"), 0, "prefix {prefix}");
        assert_recovered(fixture, notice.as_deref());
        if let Some((offset, sector)) = writes.get(prefix) {
            cut[*offset..*offset + SECTOR_BYTES].copy_from_slice(sector);
        }
    }
    assert_eq!(cut, after);
}

#[test]
fn backup_with_an_empty_journal_inode_can_be_checked_and_converted() {
    require_tools();
    let mut fixture = Fixture::new("backup-empty-journal-inode");
    format_journalled(&mut fixture);
    let mut partial = fs::read(&fixture.image.0).unwrap();
    let sb = Superblock::read(&partial).unwrap();
    let inode = sb.journal_inode(&partial).unwrap();
    let size = usize::from(sb.word(88));
    partial[inode..inode + size].fill(0);
    partial[geometry::PRIMARY.clone()].fill(0);
    assert!(matches!(geometry::source(&partial), Some(Source::Backup { .. })));
    fs::write(&fixture.image.0, partial).unwrap();

    assert_repaired(&mut fixture);
    assert!(fixture.open_card().data_has_journal().unwrap());
}

#[test]
fn conversion_preserves_files_at_every_sector_cut_and_old_fsck_accepts_it() {
    require_tools();
    let mut fixture = Fixture::new("conversion");
    fixture.format_ext2("1024", "8192");
    fixture.debugfs(&["-w"], "mkdir /cal");
    fixture.put_file("/cal/copy", b"synthetic calibration");
    fixture.put_file("/settings", b"synthetic user settings");
    let filler = vec![0x5a; (12 * 1024 - 64) * BLOCK_BYTES];
    fixture.put_file("/shots", &filler);
    let paths = ["/cal/copy", "/settings", "/shots"];
    let hashes = paths.map(|name| contents_hash(&fixture, name));
    assert_eq!(hashes[2], sha256(&filler));

    let (result, before, after) = fixture.run();
    assert_eq!(result, Ok(None));
    assert!(fixture.open_card().data_has_journal().unwrap());
    let journal = Superblock::read(&after).unwrap().journal_sector(&after).unwrap();
    assert_eq!(u32::from_be_bytes(after[journal + 16..journal + 20].try_into().unwrap()), 1024);
    assert_eq!(available(&before) - available(&after), 1029); // 1024 blocks plus five indirect blocks.
    assert!(available(&after) >= 1024);
    eprintln!(
        "journal conversion: {} KiB available after reserved blocks on the full-looking fixture",
        available(&after)
    );

    let mut writes = Vec::new();
    let conversion = WriteKind::Conversion(journal::conversion_inode(&after).unwrap());
    write_changes(&before, &after, conversion, record_sectors(&mut writes)).unwrap();
    assert_eq!(writes.last().unwrap().0, geometry::PRIMARY.start);
    eprintln!("journal conversion: checking all {} sector prefixes", writes.len() + 1);
    test_prefixes(&mut fixture, &before, &after, &writes, |fixture, notice| {
        assert_eq!(notice, None);
        assert_eq!(paths.map(|name| contents_hash(fixture, name)), hashes);
    });
    // A matching conversion write must not hide an unrelated lost directory entry.
    fs::write(&fixture.image.0, &before).unwrap();
    let mut cut = OpenOptions::new().write(true).open(&fixture.image.0).unwrap();
    let (offset, sector) = &writes[0];
    write_at(&mut cut, *offset as u64, sector);
    fixture.debugfs(&["-w"], "unlink /settings");
    assert_repaired(&mut fixture);
    assert_eq!(contents_hash(&fixture, &fixture.recovered_name()), hashes[1]);

    // K14 also had a prefix after its initial ext2 error marker. Accept that legacy cut too.
    fs::write(&fixture.image.0, before).unwrap();
    fixture.debugfs(&["-w"], "set_super_value state 2");
    let (result, _, recovered) = fixture.run();
    assert!(result.is_ok(), "{result:?}");
    fs::write(&fixture.image.0, recovered).unwrap();
    assert_eq!(fixture.fsck("-fn"), 0);
    assert_eq!(paths.map(|name| contents_hash(&fixture, name)), hashes);
    fs::write(&fixture.image.0, after).unwrap();
    for mode in ["-p", "-fy", "-fn"] {
        assert_eq!(fixture.fsck(mode), 0, "older startup checker {mode}");
    }
}

#[test]
fn low_space_leaves_the_checked_ext2_image_unchanged() {
    require_tools();
    let mut fixture = Fixture::new("space");
    fixture.format_ext2("1024", "8192");
    fixture.put_file("/shots", &vec![0x5a; (12 * 1024 + 64) * BLOCK_BYTES]);
    let (result, before, after) = fixture.run();
    assert_eq!(result, Ok(None));
    assert!(available(&before) > 1029 && available(&before) < 1029 + 1024);
    assert_eq!(before, after);
    assert!(!fixture.open_card().data_has_journal().unwrap());
}

fn dirty_journal(fixture: &Fixture) {
    let block = |name| fixture.debugfs(&[], &format!("bmap {name} 0")).trim().parse::<u32>().unwrap();
    let committed = block("/committed");
    let uncommitted = block("/uncommitted");
    let mut data = vec![0x36; BLOCK_BYTES];
    fs::write(&fixture.payload.0, &data).unwrap();
    let rejected = Image::copy_from(&[0x44; BLOCK_BYTES]).unwrap();
    let script = format!(
        "journal_open\njournal_write -b {committed} {}\njournal_write -c -b {uncommitted} {}\njournal_close\n",
        fixture.payload.0.display(),
        rejected.0.display()
    );
    let commands = Image::copy_from(script.as_bytes()).unwrap();
    let output = Command::new("debugfs").arg("-w").arg("-f").arg(&commands.0).arg(&fixture.image.0).output().unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stderr).lines().all(|line| line.starts_with("debugfs ")), "{output:?}");
    data.fill(0x21);
    assert_eq!(contents_hash(fixture, "/committed"), sha256(&data));
    assert_eq!(contents_hash(fixture, "/uncommitted"), sha256(&data));
}

fn dirty_fixture(tag: &str) -> Fixture {
    let mut fixture = Fixture::new(tag);
    format_journalled(&mut fixture);
    for name in ["/committed", "/uncommitted"] {
        fixture.put_file(name, &[0x21; BLOCK_BYTES]);
    }
    dirty_journal(&fixture);
    fixture
}

#[test]
fn startup_replays_a_pending_journal_with_a_rollback_builds_clean_primary() {
    require_tools();
    let mut fixture = dirty_fixture("rollback-replay");
    let mut dirty = fs::read(&fixture.image.0).unwrap();
    let replayed = Image::copy_from(&dirty).unwrap();
    assert_eq!(run_check(&replayed.0, &["-fy"]), Ok(Check::Clean));
    let replayed = replayed.read(LEN).unwrap();
    let flags = geometry::PRIMARY.start..geometry::PRIMARY.start + SECTOR_BYTES;
    dirty[flags.clone()].copy_from_slice(&replayed[flags]);
    assert!(!Superblock::read(&dirty).unwrap().needs_recovery());
    fs::write(&fixture.image.0, dirty).unwrap();
    assert_eq!(fixture.fsck("-p"), UNCORRECTED);
    assert_eq!(fixture.fsck("-fn"), 0);

    let (notice, _, recovered) = fixture.run();
    assert_eq!(notice, Ok(None));
    fs::write(&fixture.image.0, recovered).unwrap();
    assert_eq!(contents_hash(&fixture, "/committed"), sha256(&[0x36; BLOCK_BYTES]));
    assert_eq!(contents_hash(&fixture, "/uncommitted"), sha256(&[0x21; BLOCK_BYTES]));
}

#[test]
fn forced_replay_with_a_wrong_free_block_count_reports_an_ordinary_repair() {
    require_tools();
    let mut fixture = dirty_fixture("replay-wrong-counter");
    let mut dirty = fs::read(&fixture.image.0).unwrap();
    dirty[geometry::PRIMARY.start + 0x60] &= !0x04;
    dirty[geometry::PRIMARY.start + 12..][..4].copy_from_slice(&1u32.to_le_bytes());
    fs::write(&fixture.image.0, &dirty).unwrap();
    let probe = Image::copy_from(&dirty).unwrap();
    assert_eq!(run_check(&probe.0, &["-y", "-E", "journal_only"]), Ok(Check::Corrected));

    let (notice, _, recovered) = fixture.run();
    assert_eq!(notice.unwrap().as_deref(), Some("The data area was repaired. Damaged entries may be missing. Check your saved files and look in lost+found for recovered copies."));
    fs::write(&fixture.image.0, recovered).unwrap();
    assert_eq!(fixture.fsck("-fn"), 0);
    assert_eq!(contents_hash(&fixture, "/committed"), sha256(&[0x36; BLOCK_BYTES]));
    assert_eq!(contents_hash(&fixture, "/uncommitted"), sha256(&[0x21; BLOCK_BYTES]));
}

#[test]
fn startup_warns_when_fsck_resets_a_corrupt_journal() {
    require_tools();
    let mut fixture = dirty_fixture("corrupt-journal");
    let mut dirty = fs::read(&fixture.image.0).unwrap();
    let head = Superblock::read(&dirty).unwrap().journal_sector(&dirty).unwrap();
    // Invalidate the journal's block size while preserving its magic and pending transaction.
    dirty[head + 12..head + 16].copy_from_slice(&0u32.to_be_bytes());
    fs::write(&fixture.image.0, &dirty).unwrap();
    let probe = Image::copy_from(&dirty).unwrap();
    assert_eq!(run_check(&probe.0, &["-p", "-E", "journal_only"]), Ok(Check::Corrected));

    let (notice, _, recovered) = fixture.run();
    assert_eq!(notice.unwrap().as_deref(), Some("The data area was repaired. Recent changes may be lost because the journal was reset. Check your settings and restore missing files from backup."));
    fs::write(&fixture.image.0, recovered).unwrap();
    assert_eq!(fixture.fsck("-fn"), 0);
    assert_eq!(contents_hash(&fixture, "/committed"), sha256(&[0x21; BLOCK_BYTES]));
}

#[test]
fn startup_replays_committed_transactions_and_discards_uncommitted_ones() {
    require_tools();
    let mut fixture = dirty_fixture("replay");
    let (result, before, after) = fixture.run();
    assert_eq!(result, Ok(None));
    let dirty = Superblock::read(&before).unwrap();
    assert!(dirty.needs_recovery());
    assert!(!Superblock::read(&after).unwrap().needs_recovery());
    let journal = dirty.journal_sector(&before).unwrap();
    let mut writes = Vec::new();
    let delayed = WriteKind::Replay(journal::pending_replay(&before, Source::Primary).unwrap().unwrap());
    write_changes(&before, &after, delayed, record_sectors(&mut writes)).unwrap();
    assert_eq!(writes[writes.len() - 2].0, journal);
    assert_eq!(writes.last().unwrap().0, geometry::PRIMARY.start);
    eprintln!("journal replay: checking all {} sector prefixes", writes.len() + 1);
    test_prefixes(&mut fixture, &before, &after, &writes, |fixture, notice| {
        assert_eq!(notice, None);
        assert_eq!(contents_hash(fixture, "/committed"), sha256(&[0x36; BLOCK_BYTES]));
        assert_eq!(contents_hash(fixture, "/uncommitted"), sha256(&[0x21; BLOCK_BYTES]));
    });
    assert_eq!(fixture.fsck("-fn"), 0);
}

#[test]
fn clean_journalled_area_ignores_preen_bookkeeping() {
    require_tools();
    let mut fixture = Fixture::new("journal-clean-bookkeeping");
    format_journalled(&mut fixture);
    // The kernel records lifetime writes on a mounted filesystem.
    fixture.debugfs(&["-w"], "set_super_value kbytes_written 1234");

    let before = fs::read(&fixture.image.0).unwrap();
    assert!(journal::pending_replay(&before, Source::Primary).unwrap().is_none());
    let probe = Image::copy_from(&before).unwrap();
    assert_eq!(run_check(&probe.0, &["-p"]), Ok(Check::Clean));
    let runs = data_runs(&before, &probe.read(LEN).unwrap(), WriteKind::Repair).unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0], geometry::PRIMARY.start..geometry::PRIMARY.start + SECTOR_BYTES);

    let (notice, before, after) = fixture.run();
    assert_eq!(notice, Ok(None));
    assert!(before == after, "startup wrote back scratch-image write accounting");
    fs::write(&fixture.image.0, after).unwrap();
    assert_eq!(fixture.fsck("-fn"), 0);
}

#[test]
fn clean_flagged_journalled_area_repairs_unattached_inode_and_wrong_counters() {
    require_tools();
    let mut fixture = Fixture::new("journal-clean-flag");
    format_journalled(&mut fixture);
    let recovered = damage_files(&fixture, "1");
    assert_eq!(fixture.fsck("-p"), 0);
    assert_repaired(&mut fixture);
    assert!(fixture.open_card().data_has_journal().unwrap());
    assert_eq!(contents_hash(&fixture, &fixture.recovered_name()), sha256(recovered));
}

#[test]
fn journalled_area_recovers_after_every_prefix_of_the_real_repair() {
    require_tools();
    let mut fixture = Fixture::new("journal-interrupted");
    format_journalled(&mut fixture);
    assert_repair_prefixes(&mut fixture, true);
}

#[test]
fn journalled_area_recovers_a_torn_primary_with_and_without_pending_transactions() {
    require_tools();
    let mut fixture = Fixture::new("journal-backup-super");
    for dirty in [false, true] {
        format_journalled(&mut fixture);
        for name in ["/committed", "/uncommitted"] {
            fixture.put_file(name, &[0x21; BLOCK_BYTES]);
        }
        if dirty {
            dirty_journal(&fixture);
        }
        let mut image = OpenOptions::new().write(true).open(&fixture.image.0).unwrap();
        write_at(&mut image, geometry::PRIMARY.start as u64, &[0; SECTOR_BYTES]);
        assert_repaired(&mut fixture);
        assert!(fixture.open_card().data_has_journal().unwrap());
        let committed = if dirty { 0x36 } else { 0x21 };
        assert_eq!(contents_hash(&fixture, "/committed"), sha256(&[committed; BLOCK_BYTES]));
        assert_eq!(contents_hash(&fixture, "/uncommitted"), sha256(&[0x21; BLOCK_BYTES]));
    }
}

#[test]
fn uncorrected_forced_replay_runs_forced_repair_and_verifies_it() {
    let replay = journal::pending_replay(&pending_journal_image(), Source::Primary).unwrap().unwrap();
    for verdict in [Check::Clean, Check::Uncorrected { corrected: false }] {
        let mut checks = [
            (&["-y", "-E", "journal_only"][..], Check::Uncorrected { corrected: true }),
            (&["-fy"][..], Check::Corrected),
            (&["-fn"][..], verdict),
        ]
        .into_iter();
        let result = check_until_clean(Source::Primary, Some(replay), |args| {
            let (expected, status) = checks.next().unwrap();
            assert_eq!(args, expected);
            Ok(status)
        });
        if verdict == Check::Clean {
            assert_eq!(result, Ok(true));
        } else {
            assert!(result.unwrap_err().contains("read-only check"));
        }
        assert_eq!(checks.count(), 0);
    }
}
