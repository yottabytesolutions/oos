// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

fn staged(name: &str, count: usize) -> (std::path::PathBuf, Vec<String>) {
    let dir = std::env::temp_dir().join(format!("oos-calfile-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let paths: Vec<_> = (0..count).map(|k| dir.join(format!("{k}.hex")).to_str().unwrap().to_string()).collect();
    for path in &paths {
        std::fs::write(path, b"good").unwrap();
        std::fs::write(format!("{path}.bak"), b"previous").unwrap();
        std::fs::write(format!("{path}.new"), b"next").unwrap();
    }
    (dir, paths)
}

#[test]
fn audit0073_publish_always_keeps_the_live_file() {
    let (dir, paths) = staged("atomic", 1);
    let result = publish_with(&paths, |from, to| {
        if from.ends_with(".new") {
            assert_eq!(std::fs::read(to).unwrap(), b"good");
            assert_eq!(std::fs::read(format!("{to}.bak")).unwrap(), b"good");
        }
        std::fs::rename(from, to)
    });
    assert!(result.is_ok());
    assert_eq!(std::fs::read(&paths[0]).unwrap(), b"next");
    assert_eq!(std::fs::read(format!("{}.bak", paths[0])).unwrap(), b"good");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn audit0073_rollback_attempts_all_restores_and_combines_errors() {
    let (dir, paths) = staged("restore-errors", 4);
    let mut restores = Vec::new();
    let result = publish_with(&paths, |from, to| {
        if from == format!("{}.new", paths[3]) {
            return Err(std::io::Error::other("publish failed"));
        }
        if from.ends_with(".bak") {
            restores.push(to.to_string());
            for (k, error) in [(1, "restore one failed"), (2, "restore two failed")] {
                if to == paths[k] {
                    return Err(std::io::Error::other(error));
                }
            }
        }
        std::fs::rename(from, to)
    });
    let error = result.unwrap_err().to_string();
    let first = std::fs::read(&paths[0]);
    std::fs::remove_dir_all(dir).unwrap();
    assert!(error.contains("publish failed"), "{error}");
    assert!(error.contains("restore one failed"), "{error}");
    assert!(error.contains("restore two failed"), "{error}");
    assert!(restores.contains(&paths[0]));
    assert_eq!(first.unwrap(), b"good");
}

#[test]
fn audit0073_successful_publish_reports_directory_sync_failure() {
    let (dir, paths) = staged("dir-sync", 1);
    let result = save_all_with_sync(dir.to_str().unwrap(), &[("0.hex", b"saved")], (2000, 1, 1, 0, 0, 0), |_| {
        Err(std::io::Error::other("directory sync failed"))
    });
    assert_eq!(parse(&std::fs::read(&paths[0]).unwrap()), Ok(&b"saved"[..]));
    std::fs::remove_dir_all(dir).unwrap();
    let error = result.unwrap_err();
    assert!(error.to_string().contains("files published; directory fsync failed"));
    assert!(matches!(error, SaveError::PublishedDirectorySync(e) if e.to_string() == "directory sync failed"));
}

#[test]
fn audit0073_backup_link_failure_preserves_previous_backup() {
    let (dir, paths) = staged("backup-link-failure", 1);
    let result = backup_with(&paths[0], |_, _| Err(std::io::Error::other("link failed")));
    let live = std::fs::read(&paths[0]);
    let backup = std::fs::read(format!("{}.bak", paths[0]));
    let temporary = std::path::Path::new(&format!("{}.bak.new", paths[0])).exists();
    std::fs::remove_dir_all(dir).unwrap();
    assert_eq!(result.unwrap_err().to_string(), "link failed");
    assert_eq!(live.unwrap(), b"good");
    assert_eq!(backup.unwrap(), b"previous");
    assert!(!temporary);
}

#[test]
fn stale_backup_link_does_not_block_save() {
    let (dir, paths) = staged("stale-backup", 1);
    let temporary = format!("{}.bak.new", paths[0]);
    std::fs::hard_link(&paths[0], &temporary).unwrap();
    let result = save(dir.to_str().unwrap(), "0.hex", b"saved", (2000, 1, 1, 0, 0, 0));
    let live = std::fs::read(&paths[0]);
    let backup = std::fs::read(format!("{}.bak", paths[0]));
    let temporary_exists = std::path::Path::new(&temporary).exists();
    let staged_exists = std::path::Path::new(&format!("{}.new", paths[0])).exists();
    std::fs::remove_dir_all(dir).unwrap();
    result.unwrap();
    assert_eq!(parse(&live.unwrap()), Ok(&b"saved"[..]));
    assert_eq!(backup.unwrap(), b"good");
    assert!(!temporary_exists);
    assert!(!staged_exists);
}

#[test]
fn audit0073_backup_links_before_replacing_previous_backup() {
    let (dir, paths) = staged("backup-link-order", 1);
    backup_with(&paths[0], |from, to| {
        assert_eq!(std::fs::read(format!("{from}.bak")).unwrap(), b"previous");
        assert_eq!(to, format!("{from}.bak.new"));
        std::fs::hard_link(from, to)
    })
    .unwrap();
    assert_eq!(std::fs::read(&paths[0]).unwrap(), b"good");
    assert_eq!(std::fs::read(format!("{}.bak", paths[0])).unwrap(), b"good");
    assert!(!std::path::Path::new(&format!("{}.bak.new", paths[0])).exists());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn audit0073_publish_failure_is_not_a_published_sync_warning() {
    let (dir, paths) = staged("publish-sync-failure", 1);
    std::fs::remove_file(format!("{}.bak", paths[0])).unwrap();
    std::fs::create_dir(format!("{}.bak", paths[0])).unwrap();
    let result = save_all_with_sync(dir.to_str().unwrap(), &[("0.hex", b"saved")], (2000, 1, 1, 0, 0, 0), |_| {
        Err(std::io::Error::other("directory sync failed"))
    });
    assert_eq!(std::fs::read(&paths[0]).unwrap(), b"good");
    assert!(!std::path::Path::new(&format!("{}.bak.new", paths[0])).exists());
    assert!(!std::path::Path::new(&format!("{}.new", paths[0])).exists());
    std::fs::remove_dir_all(dir).unwrap();
    assert!(matches!(result, Err(SaveError::Failed(e)) if e.to_string().contains("directory fsync")));
}

#[test]
fn audit0073_save_restores_files_after_late_rename_error() {
    let dir = std::env::temp_dir().join(format!("oos-calfile-rollback-{}", std::process::id()));
    let ds = dir.to_str().unwrap();
    let stamp = (2000, 1, 1, 0, 0, 0);
    save(ds, "first.hex", &[1], stamp).unwrap();
    save(ds, "last.hex", &[2], stamp).unwrap();
    std::fs::create_dir(dir.join("last.hex.bak")).unwrap();
    assert!(save_all(ds, &[("first.hex", &[3]), ("new.hex", &[4]), ("last.hex", &[5])], stamp).is_err());
    assert_eq!(parse(&std::fs::read(dir.join("first.hex")).unwrap()), Ok(&[1][..]));
    assert_eq!(parse(&std::fs::read(dir.join("last.hex")).unwrap()), Ok(&[2][..]));
    assert!(!dir.join("new.hex").exists());
    assert!(!dir.join("first.hex.new").exists());
    assert!(!dir.join("last.hex.new").exists());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn utc_stamps_use_explicit_seconds() {
    for (secs, want) in [
        (0, (1970, 1, 1, 0, 0, 0)),
        (951_827_696, (2000, 2, 29, 12, 34, 56)),
        (1_762_862_400, (2025, 11, 11, 12, 0, 0)),
    ] {
        assert_eq!(stamp_at(secs), want);
    }
}

#[test]
fn data_partition_mounted() {
    assert!(oos_mounted("rootfs / rootfs rw 0 0\n/dev/mmcblk0p1 /oos ext4 rw,noatime 0 0\n"));
    assert!(!oos_mounted(
        "rootfs / rootfs rw 0 0\n/dev/sda1 /oos/usb vfat rw 0 0\n/dev/mmcblk0p9 /rigol ext4 ro 0 0\n"
    ));
}

#[test]
fn synthetic_files_parse_and_rebuild_byte_for_byte() {
    for (name, bytes) in crate::dho::fixtures::files() {
        let payload = parse(&bytes).unwrap_or_else(|e| panic!("{}: {:?}", name, e));
        assert_eq!(build(payload, stamp(&bytes).unwrap()), bytes, "{}", name);
        assert_eq!(stamp(&bytes), Some((2000, 1, 1, 0, 0, 0)));
    }
}

#[test]
#[ignore = "private calibration: set OOS_CAL_DIR"]
fn private_files_parse_and_rebuild_byte_for_byte() {
    for name in crate::dho::fixtures::NAMES {
        // Explicit opt-in requires every file, so a missing scenario fails.
        let bytes = crate::dho::fixtures::private(name);
        let payload = parse(&bytes).unwrap_or_else(|e| panic!("{}: {:?}", name, e));
        assert_eq!(build(payload, stamp(&bytes).unwrap()), bytes, "{}", name);
    }
}

#[test]
fn errors_and_save_refuses_rigol() {
    let b = build(&[1, 2, 3, 4], (2026, 11, 3, 21, 5, 9));
    assert_eq!(parse(&b), Ok(&[1u8, 2, 3, 4][..]));
    assert_eq!(stamp(&b), Some((2026, 11, 3, 21, 5, 9)));
    let mut bad = b.clone();
    bad[30] ^= 1;
    assert_eq!(parse(&bad), Err(Error::PayloadCrc));
    bad = b.clone();
    bad[12] ^= 1;
    assert_eq!(parse(&bad), Err(Error::HeaderCrc));
    assert_eq!(parse(&b[..20]), Err(Error::Short));
    assert!(save("/rigol/data", "cal_vertical.hex", &[0], (2000, 1, 1, 0, 0, 0)).is_err());
    assert!(save("/rigol//data/", "x", &[0], (2000, 1, 1, 0, 0, 0)).is_err());
    assert!(save("/tmp/../rigol/data", "x", &[0], (2000, 1, 1, 0, 0, 0)).is_err());
    // atomic save with .bak
    let d = std::env::temp_dir().join(format!("oos-calfile-{}", std::process::id()));
    let ds = d.to_str().unwrap();
    save(ds, "c.hex", &[7; 10], (2000, 1, 1, 0, 0, 0)).unwrap();
    save(ds, "c.hex", &[8; 10], (2000, 1, 1, 0, 0, 0)).unwrap();
    assert_eq!(parse(&std::fs::read(d.join("c.hex")).unwrap()), Ok(&[8u8; 10][..]));
    assert_eq!(parse(&std::fs::read(d.join("c.hex.bak")).unwrap()), Ok(&[7u8; 10][..]));
    let missing = d.join("missing");
    let (_, from) = load_first("c.hex", &[missing.to_str().unwrap(), ds]).unwrap();
    assert_eq!(from, ds);
    std::fs::remove_dir_all(&d).unwrap();
}
