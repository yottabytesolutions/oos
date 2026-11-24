// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use std::io::Read;
use std::os::unix::process::ExitStatusExt;

#[test]
fn fsck_exit_bits_preserve_corrections_and_reject_tool_failures() {
    let check = |number| check_result(ExitStatus::from_raw(number << 8));
    assert_eq!(check(0), Ok(Check::Clean));
    for bits in [1, 2, 3] {
        assert_eq!(check(bits), Ok(Check::Corrected));
    }
    for bits in [4, 5, 6, 7] {
        assert_eq!(check(bits), Ok(Check::Uncorrected { corrected: bits != UNCORRECTED }));
    }
    for (bit, reason) in [(8, "operational"), (16, "usage"), (32, "cancelled"), (64, "unknown"), (128, "library")] {
        assert!(check(bit).unwrap_err().contains(reason));
        assert!(check(bit | CORRECTED | UNCORRECTED).unwrap_err().contains(reason));
    }
    assert!(check_result(ExitStatus::from_raw(9)).unwrap_err().contains("interrupted"));
}

#[test]
fn read_only_verdict_controls_escalation_and_acceptance() {
    for preen in [Ok(Check::Clean), Ok(Check::Corrected), Err("preen failed".into())] {
        for verified in [Ok(Check::Clean), Ok(Check::Uncorrected { corrected: false }), Err("check failed".into())] {
            let repair = verified != Ok(Check::Clean);
            let mut checks = [
                ("-p", preen.clone()),
                ("-fn", verified),
                ("-fy", Err("repair failed".into())),
                ("-fn", Ok(Check::Clean)),
            ]
            .into_iter();
            assert_eq!(
                check_until_clean(Source::Primary, |args| {
                    let (expected, result) = checks.next().unwrap();
                    assert_eq!(args, [expected]);
                    result
                }),
                Ok(())
            );
            assert_eq!(checks.count(), if repair { 0 } else { 2 });
        }
    }
    let mut checks = [
        Check::Corrected,
        Check::Uncorrected { corrected: false },
        Check::Corrected,
        Check::Uncorrected { corrected: false },
    ]
    .into_iter();
    assert!(check_until_clean(Source::Primary, |_| Ok(checks.next().unwrap())).is_err());
    assert_eq!(checks.count(), 0);
}

const START: u64 = OOSDATA_LBA * SECTOR;
const LEN: usize = (crate::target::ACTIVE.layout.oosdata_sectors * SECTOR) as usize;
const END: u64 = START + LEN as u64;

struct Fixture {
    card: Image,
    image: Image,
    payload: Image,
    file: fs::File,
}

impl Fixture {
    fn new(tag: &str) -> Self {
        let path = |kind| Image(std::env::temp_dir().join(format!("oos-data-{tag}-{kind}-{}", std::process::id())));
        let card = path("fsck-card");
        let image = path("fsck-image");
        let payload = path("fsck-payload");
        let mut file = OpenOptions::new().write(true).read(true).create_new(true).open(&card.0).unwrap();
        file.set_len(END + 1).unwrap();
        write_at(
            &mut file,
            crate::update::card::PARM_LBA * SECTOR,
            crate::target::ACTIVE.parm_marks.join(" ").as_bytes(),
        );
        write_at(&mut file, START - 1, &[91]);
        write_at(&mut file, END, &[92]);
        fs::File::create(&image.0).unwrap().set_len(LEN as u64).unwrap();
        Self { card, image, payload, file }
    }

    fn open_card(&self) -> Card {
        Card::open(&self.card.0, true).unwrap()
    }

    fn run(&mut self) -> (Result<Option<String>, String>, Vec<u8>, Vec<u8>) {
        let before = fs::read(&self.image.0).unwrap();
        write_at(&mut self.file, START, &before);
        let mut card = self.open_card();
        let result = card.prepare_data();
        let after = card.read(OOSDATA_LBA, LEN).unwrap();
        self.assert_bounds();
        (result, before, after)
    }

    fn assert_bounds(&mut self) {
        for (offset, value) in [(START - 1, 91), (END, 92)] {
            let mut byte = [0];
            self.file.seek(SeekFrom::Start(offset)).unwrap();
            self.file.read_exact(&mut byte).unwrap();
            assert_eq!(byte, [value]);
        }
    }

    fn debugfs(&self, args: &[&str], request: &str) -> String {
        String::from_utf8(self.debugfs_bytes(args, request)).unwrap()
    }

    fn debugfs_bytes(&self, args: &[&str], request: &str) -> Vec<u8> {
        let output = Command::new("debugfs").args(args).args(["-R", request]).arg(&self.image.0).output().unwrap();
        assert!(output.status.success(), "{request}: {:?}", output);
        output.stdout
    }

    fn fsck(&self, mode: &str) -> i32 {
        // Probe the input on another copy so our own check cannot modify the fixture.
        let image = Image::copy_from(&fs::read(&self.image.0).unwrap()).unwrap();
        image_tool("e2fsck", &[mode], &image.0).unwrap().code().unwrap()
    }

    fn format_ext2(&self, block_size: &str, blocks_per_group: &str) {
        fs::File::create(&self.image.0).unwrap().set_len(LEN as u64).unwrap();
        assert!(image_tool("mke2fs", &["-F", "-t", "ext2", "-b", block_size, "-g", blocks_per_group], &self.image.0)
            .unwrap()
            .success());
    }

    fn put_file(&self, path: &str, data: &[u8]) {
        fs::write(&self.payload.0, data).unwrap();
        self.debugfs(&["-w"], &format!("write {} {path}", self.payload.0.display()));
    }

    fn recovered_name(&self) -> String {
        let entries = self.debugfs(&[], "ls /lost+found");
        let names: Vec<_> = entries.split_whitespace().filter(|name| name.starts_with('#')).collect();
        assert_eq!(names.len(), 1, "{entries}");
        format!("/lost+found/{}", names[0])
    }

    fn file_hashes(&self, paths: &[&str]) -> Vec<[u8; 32]> {
        paths.iter().map(|path| crate::update::hash::sha256(&self.debugfs_bytes(&[], &format!("cat {path}")))).collect()
    }
}

fn write_at(file: &mut fs::File, offset: u64, data: &[u8]) {
    file.seek(SeekFrom::Start(offset)).unwrap();
    file.write_all(data).unwrap();
}

fn require_tools() {
    for tool in ["mke2fs", "e2fsck", "debugfs"] {
        assert!(
            Command::new(tool).arg("-V").output().is_ok_and(|out| out.status.success()),
            "cannot run {tool} -V: install e2fsprogs and put its sbin directory in PATH"
        );
    }
}

fn assert_repaired(fixture: &mut Fixture) {
    let (result, before, after) = fixture.run();
    assert_ne!(before, after);
    assert_eq!(result.unwrap().unwrap(), "The data area was repaired");
    fs::write(&fixture.image.0, after).unwrap();
    assert_eq!(fixture.fsck("-fn"), 0);
}

#[test]
fn ext2_images_cover_clean_repairs_and_refusals() {
    require_tools();
    let mut fixture = Fixture::new("real");
    let (result, before, after) = fixture.run();
    assert_eq!(result, Ok(None)); // First use does not report a repair.
    assert_ne!(before, after);
    fs::write(&fixture.image.0, after).unwrap();
    assert_eq!(fixture.fsck("-fn"), 0);

    let (result, before, after) = fixture.run();
    assert_eq!(result, Ok(None));
    assert_eq!(before, after);

    // Fix the block size and backup-superblock location for the corruption cases below.
    fixture.format_ext2("1024", "8192");
    fixture.debugfs(&["-w"], "set_super_value free_blocks_count 1");
    fixture.debugfs(&["-w"], "set_super_value state 2");
    assert_eq!(fixture.fsck("-p"), 1);
    assert_repaired(&mut fixture);

    fixture.put_file("/oos-ui.setup.new", b"keep this recovered setup\n");
    fixture.debugfs(&["-w"], "unlink /oos-ui.setup.new");
    fixture.debugfs(&["-w"], "set_super_value state 1");
    assert_eq!(fixture.fsck("-p"), 0); // Preen takes the clean flag at face value.
    assert_repaired(&mut fixture);
    assert_eq!(fixture.debugfs(&[], &format!("cat {}", fixture.recovered_name())), "keep this recovered setup\n");

    let mut image = OpenOptions::new().write(true).open(&fixture.image.0).unwrap();
    // Default mke2fs on this fixture has at most one backup, at block 8193 for 1 KiB blocks.
    write_at(&mut image, 8193 * 1024, &[0; 1024]);
    write_at(&mut image, 1024, &[0; 1024]);
    write_at(&mut image, 1080, &[0x53, 0xef]); // Exercise the fsck refusal even with an ext signature.
    let (result, before, after) = fixture.run();
    assert!(result.unwrap_err().contains("e2fsck"));
    assert_eq!(before, after);

    fs::write(&fixture.image.0, vec![0x5a; LEN]).unwrap();
    let (result, before, after) = fixture.run();
    assert!(result.unwrap_err().contains("neither ext nor empty"));
    assert_eq!(before, after);
}

#[test]
fn ext2_recovers_after_every_prefix_of_the_real_repair() {
    require_tools();
    let mut fixture = Fixture::new("interrupted");
    fixture.format_ext2("1024", "8192");
    fixture.debugfs(&["-w"], "mkdir /setups");
    fixture.put_file("/setups/keep", b"saved setup\n");
    fixture.put_file("/oos-ui.setup", b"current setup\n");
    let recovered = b"keep this recovered setup\n";
    fixture.put_file("/oos-ui.setup.new", recovered);
    fixture.debugfs(&["-w"], "unlink /oos-ui.setup.new");
    fixture.debugfs(&["-w"], "set_bg 0 free_blocks_count 1");
    fixture.debugfs(&["-w"], "set_super_value free_blocks_count 1");
    fixture.debugfs(&["-w"], "set_super_value free_inodes_count 1");
    fixture.debugfs(&["-w"], "set_super_value state 2");
    assert_ne!(fixture.fsck("-p") & UNCORRECTED, 0);

    let (result, damaged, repaired) = fixture.run();
    assert_eq!(result.unwrap().as_deref(), Some("The data area was repaired"));
    fs::write(&fixture.image.0, &repaired).unwrap();
    let name = fixture.recovered_name();
    let paths = ["/setups/keep", "/oos-ui.setup", &name];
    let hashes = fixture.file_hashes(&paths);
    assert_eq!(hashes[2], crate::update::hash::sha256(recovered));
    assert_eq!(fixture.fsck("-fn"), 0);

    let runs = data_runs(&damaged, &repaired).unwrap();
    assert!(runs.len() >= 3);
    assert!(geometry::PRIMARY.contains(&runs.last().unwrap().start));
    for (index, run) in runs.iter().enumerate() {
        write_at(&mut fixture.file, START, &damaged);
        let mut card = fixture.open_card();
        card.write_fault_at = Some((START + run.start as u64, super::super::super::WriteFault::CutAfterReadBack));
        assert_eq!(card.prepare_data(), Err("power cut after read-back".into()), "prefix {}", index + 1);
        drop(card);
        let mut card = fixture.open_card();
        assert!(card.prepare_data().is_ok(), "prefix {} refused", index + 1);
        fs::write(&fixture.image.0, card.read(OOSDATA_LBA, LEN).unwrap()).unwrap();
        assert_eq!(fixture.fsck("-fn"), 0, "prefix {}", index + 1);
        assert_eq!(fixture.file_hashes(&paths), hashes, "prefix {}", index + 1);
        fixture.assert_bounds();
    }
    eprintln!("checked all {} write-back prefixes", runs.len());
}

#[test]
fn ext2_repairs_a_zeroed_primary_superblock_from_its_backup() {
    require_tools();
    let mut fixture = Fixture::new("backup-super");
    for block_size in ["1024", "2048", "4096"] {
        fixture.format_ext2(block_size, "2048");
        let setup = b"keep the saved setup\n";
        fixture.put_file("/oos-ui.setup", setup);
        let mut image = OpenOptions::new().write(true).open(&fixture.image.0).unwrap();
        if block_size == "4096" {
            // The formatter shipped on the unit writes zero for the backup's group number.
            write_at(&mut image, 2048 * 4096 + 0x5a, &[0; 2]);
        }
        write_at(&mut image, geometry::PRIMARY.start as u64, &[0; SECTOR as usize]);
        assert_repaired(&mut fixture);
        assert_eq!(fixture.file_hashes(&["/oos-ui.setup"]), [crate::update::hash::sha256(setup)]);
    }
}

#[test]
fn changed_data_precedes_all_superblock_and_descriptor_runs() {
    let mut after = vec![0; 8192];
    for (offset, bytes) in [(4, 8u32.to_le_bytes()), (0x14, 1u32.to_le_bytes()), (0x20, 4u32.to_le_bytes())] {
        after[1024 + offset..1024 + offset + 4].copy_from_slice(&bytes);
    }
    after[1080..1082].copy_from_slice(&[0x53, 0xef]);
    let before = vec![0xff; after.len()];
    assert_eq!(
        data_runs(&before, &after).unwrap(),
        [0..1024, 3072..5120, 7168..8192, 2048..3072, 5120..7168, 1024..2048]
    );
}

#[test]
fn repair_image_never_changes_other_areas_and_failures_do_not_write() {
    let mut fixture = Fixture::new("fake");
    let mut card = fixture.open_card();
    assert!(card
        .repair_data(|image, _| {
            fs::write(image, [99]).unwrap();
            Err("failed repair".into())
        })
        .is_err());
    assert!(card.read(OOSDATA_LBA, 1082).unwrap().iter().all(|&b| b == 0));
    card.repair_data(|image, source| {
        assert_eq!(source, Source::Empty);
        let mut f = OpenOptions::new().write(true).open(image).unwrap();
        write_at(&mut f, 0, &[17]);
        write_at(&mut f, 1028, &((LEN / 1024) as u32).to_le_bytes());
        write_at(&mut f, 1056, &8192u32.to_le_bytes());
        write_at(&mut f, 1080, &[0x53, 0xef]);
        write_at(&mut f, LEN as u64 - 1, &[18]);
        Ok(())
    })
    .unwrap();
    let data = card.read(OOSDATA_LBA, 1082).unwrap();
    assert_eq!(data[0], 17);
    assert_eq!(&data[1080..], &[0x53, 0xef]);
    assert_eq!(card.read(OOSDATA_LBA + crate::target::ACTIVE.layout.oosdata_sectors - 1, 512).unwrap()[511], 18);
    card.repair_data(|_, source| {
        assert_eq!(source, Source::Primary);
        Ok(())
    })
    .unwrap();
    assert!(card
        .repair_data(|image, _| {
            fs::write(image, [0]).unwrap();
            Ok(())
        })
        .is_err());
    fixture.assert_bounds();
}
