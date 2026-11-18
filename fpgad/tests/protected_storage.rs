// SPDX-License-Identifier: GPL-2.0-or-later
use oos_hal::target::ACTIVE;
use oos_hal::update::card::{BACKUP_LBA, BOOT_LBA, OOSDATA_LBA, PARM_LBA, RECOVERY_LBA, SECTOR};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::process::{Command, Stdio};

#[test]
fn boot_helpers_write_allowed_ranges_and_refuse_protected_targets() {
    let path = std::env::temp_dir().join(format!("oos-storage-cli-{}", std::process::id()));
    let mut file = OpenOptions::new().read(true).write(true).create_new(true).open(&path).expect("test fixture");
    file.set_len((OOSDATA_LBA + ACTIVE.layout.oosdata_sectors) * SECTOR).expect("test fixture");
    file.seek(SeekFrom::Start(PARM_LBA * SECTOR)).expect("test fixture");
    file.write_all(ACTIVE.parm_marks.join(" ").as_bytes()).expect("test fixture");
    for card_option in ["--card", "--host-card"] {
        let write = |area: &str, offset: u64, data: &[u8]| {
            let mut child = Command::new(env!("CARGO_BIN_EXE_oos-update"))
                .args(["storage-write", area, &offset.to_string(), card_option])
                .arg(&path)
                .arg("--execute")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .expect("test fixture");
            let _ = child.stdin.take().expect("test fixture").write_all(data);
            child.wait_with_output().expect("test fixture")
        };
        for (area, lba) in
            [("boot", BOOT_LBA), ("recovery", RECOVERY_LBA), ("backup", BACKUP_LBA), ("backup", OOSDATA_LBA)]
        {
            let out = write(area, lba * SECTOR, b"KRNL");
            assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
            assert_eq!(read(&mut file, lba * SECTOR, 4), b"KRNL");
        }
        assert!(!write("backup", BACKUP_LBA * SECTOR, &vec![0; oos_hal::update::card::IMAGE_MAX + 2]).status.success());
        assert_eq!(read(&mut file, BACKUP_LBA * SECTOR, 4), b"KRNL");
        let before = read(&mut file, BOOT_LBA * SECTOR, 512);
        for area in
            ["misc", "bcb", "uboot", "p4", "p5", "fram", "mtd0", "fpga", "/dev/block/by-name/uboot", "/dev/mmcblk0p5"]
        {
            assert!(!write(area, BOOT_LBA * SECTOR, b"bad!").status.success(), "{area}");
        }
        for offset in [0, 0x400000, 0x7fffff, 0x800000, BOOT_LBA * SECTOR + 1] {
            assert!(!write("boot", offset, b"bad!").status.success());
        }
        assert!(!write("boot", BOOT_LBA * SECTOR, &[0; 512]).status.success());
        assert_eq!(read(&mut file, BOOT_LBA * SECTOR, 512), before);
    }
    std::fs::remove_file(path).expect("test fixture");
}

fn read(file: &mut File, offset: u64, len: usize) -> Vec<u8> {
    file.seek(SeekFrom::Start(offset)).expect("test fixture");
    let mut data = vec![0; len];
    file.read_exact(&mut data).expect("test fixture");
    data
}

#[test]
fn host_card_selection_rejects_non_media_and_conflicting_targets() {
    let run = |targets: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_oos-update"))
            .args(["storage-write", "boot", &(BOOT_LBA * SECTOR).to_string(), "--execute"])
            .args(targets)
            .output()
            .expect("test fixture")
    };
    for option in ["--card", "--host-card"] {
        let output = run(&[option, "/dev/null"]);
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("card open:"));
    }
    let output = run(&["--card", "card.img", "--host-card", "other.img"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("select either --card or --host-card"));
}
