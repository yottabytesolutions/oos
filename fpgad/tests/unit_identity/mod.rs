// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

// A real gzip/newc ramdisk so the signer must inspect the payload, independent of the host.
fn image_unit(unit: Option<&str>) -> Vec<u8> {
    let fields: Vec<_> = unit.map(|u| ("etc/oos-unit", 0o100644, u.as_bytes())).into_iter().collect();
    let archive = kexec::newc(&fields);
    let n = archive.len() as u16;
    let mut rd = vec![0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 0, 3, 1];
    rd.extend_from_slice(&n.to_le_bytes());
    rd.extend_from_slice(&(!n).to_le_bytes());
    rd.extend_from_slice(&archive);
    rd.extend_from_slice(&[0; 8]);
    rd.resize(rd.len().div_ceil(4) * 4, 0);
    let mut img = image();
    img.resize(4096 + rd.len().div_ceil(2048) * 2048, 0);
    img[16..20].copy_from_slice(&(rd.len() as u32).to_le_bytes());
    img[4096..4096 + rd.len()].copy_from_slice(&rd);
    let id = hash::sha1(&[
        &img[2048..2112],
        &64u32.to_le_bytes(),
        &rd,
        &(rd.len() as u32).to_le_bytes(),
        &[],
        &0u32.to_le_bytes(),
        &img[32..576],
    ]);
    img[576..596].copy_from_slice(&id);
    img
}

#[test]
fn pack_uses_the_image_unit_and_accepts_only_a_matching_explicit_unit() {
    let f = Fixture::new();
    let key = f.keygen("private");
    for unit in [None, Some("test-unit"), Some("none")] {
        std::fs::write(f.path("image"), image_unit(unit)).expect("test setup");
        for explicit in [false, true] {
            let mut c = f.pack();
            c.arg("--key").arg(f.path("private")).arg("--untrusted-ok");
            if explicit {
                c.args(["--unit", unit.unwrap_or("none")]);
            }
            success(&c.output().expect("test setup"));
            let bytes = std::fs::read(f.path("package")).expect("test setup");
            let (h, _) = pkg::verify(&bytes, &[key]).expect("test setup");
            assert_eq!(oos_hal::update::unit::package(&h.manifest).expect("test setup"), unit.unwrap_or("none"));
        }
        refused(&f.test_pack().args(["--unit", "other-unit"]).output().expect("test setup"), "refused");
    }
}

#[test]
fn pack_refuses_invalid_or_unreadable_embedded_units() {
    let f = Fixture::new();
    for unit in ["", "two words", "../unit"] {
        std::fs::write(f.path("image"), image_unit(Some(unit))).expect("test setup");
        refused(&f.test_pack().output().expect("test setup"), "unit must be");
        assert!(!f.path("package").exists());
    }
    let mut img = image_unit(Some("test-unit"));
    img[4096] = 0;
    let rs = u32::from_le_bytes(img[16..20].try_into().expect("test setup"));
    let id = hash::sha1(&[
        &img[2048..2112],
        &64u32.to_le_bytes(),
        &img[4096..4096 + rs as usize],
        &rs.to_le_bytes(),
        &[],
        &0u32.to_le_bytes(),
        &img[32..576],
    ]);
    img[576..596].copy_from_slice(&id);
    std::fs::write(f.path("image"), img).expect("test setup");
    refused(&f.test_pack().output().expect("test setup"), "not gzip");
}

#[test]
fn signer_uses_the_receipted_build_unit_instead_of_the_current_environment() {
    let f = Fixture::new();
    let mut cmd = f.script();
    let sidecar = f.path("selected target/image/oos-dev.unit");
    std::fs::write(&sidecar, "built-unit\n").expect("test setup");
    std::fs::write(f.path("selected target/image/oos-dev.img"), image_unit(Some("built-unit"))).expect("test setup");
    f.stamp_image(cmd.get_envs().find(|(k, _)| k == &"PATH").expect("test setup").1.expect("test setup"));
    success(
        &cmd.env("OOS_SIGNING_KEY", f.path("test signing key"))
            .env("OOS_UNIT", "wrong-unit")
            .output()
            .expect("test setup"),
    );
    let bytes = std::fs::read(f.path("selected target/image/oos-dev.oosupd")).expect("test setup");
    let (h, _) = pkg::verify(&bytes, &[test_key()]).expect("test setup");
    assert_eq!(oos_hal::update::unit::package(&h.manifest).expect("test setup"), "built-unit");
}

#[test]
fn signer_refuses_missing_or_replaced_unit_sidecars() {
    for missing in [false, true] {
        let f = Fixture::new();
        let mut cmd = f.script();
        let sidecar = f.path("selected target/image/oos-dev.unit");
        if missing {
            std::fs::remove_file(sidecar).expect("test setup");
        } else {
            std::fs::write(sidecar, "other-unit\n").expect("test setup");
        }
        refused(&cmd.env("OOS_SIGNING_KEY", f.path("test signing key")).output().expect("test setup"), "oos-dev.unit");
        assert!(!f.path("selected target/image/oos-dev.oosupd").exists());
    }
}

fn simulated_card(f: &Fixture) {
    use std::io::{Seek, SeekFrom, Write};
    let mut file = std::fs::File::create(f.path("card")).expect("test setup");
    file.set_len((card::OOSDATA_LBA + 0x8000) * card::SECTOR).expect("test setup");
    file.seek(SeekFrom::Start(card::PARM_LBA * card::SECTOR)).expect("test setup");
    let marks = oos_hal::target::ACTIVE.parm_marks.join(" ");
    file.write_all(marks.as_bytes()).expect("test setup");
    file.seek(SeekFrom::Start(card::BOOT_LBA * card::SECTOR)).expect("test setup");
    file.write_all(b"KRNL\0\x10\0\0").expect("test setup");
}

#[test]
fn cli_refuses_foreign_units_and_logs_an_explicit_override() {
    let f = Fixture::new();
    simulated_card(&f);
    let key = f.keygen("private");
    std::fs::write(f.path("public"), hash::hex(&key)).expect("test setup");
    let img = image_unit(Some("other-unit"));
    std::fs::write(f.path("image"), &img).expect("test setup");
    success(
        &f.pack()
            .args(["--unit", "other-unit", "--untrusted-ok", "--key"])
            .arg(f.path("private"))
            .output()
            .expect("test setup"),
    );
    let install = || {
        let mut cmd = f.command();
        cmd.arg("install")
            .arg(f.path("package"))
            .arg("--card")
            .arg(f.path("card"))
            .arg("--state")
            .arg(f.path("state"))
            .arg("--keys")
            .arg(f.path("public"));
        cmd
    };
    refused(&install().arg("--execute").output().expect("test setup"), "--any-unit");
    assert!(!f.path("state").exists());
    let output = install().args(["--execute", "--any-unit"]).output().expect("test setup");
    success(&output);
    let log = String::from_utf8(output.stdout).expect("test setup");
    assert!(log.contains("--any-unit override") && log.contains("other-unit") && log.contains("none"));
    let mut c = card::Card::open(&f.path("card"), false).expect("test setup");
    assert_eq!(c.boot_magic().expect("test setup"), *b"OOSK");
    assert_eq!(c.read_image(card::Region::Recovery, img.len()).expect("test setup").0, img);
}

#[test]
fn pack_refuses_a_profile_that_the_plain_image_does_not_contain() {
    let f = Fixture::new();
    refused(&f.test_pack().args(["--unit", "dho804"]).output().expect("test setup"), "image unit 'none'");
    assert!(!f.path("package").exists());
}

#[test]
fn card_file_install_uses_its_explicit_simulated_unit() {
    let f = Fixture::new();
    simulated_card(&f);
    std::fs::write(f.path("image"), image_unit(Some("fake-unit"))).expect("test setup");
    success(&f.test_pack().output().expect("test setup"));
    let install = || {
        let mut c = f.command();
        c.arg("install")
            .arg(f.path("package"))
            .arg("--keys")
            .arg(f.path("test public key"))
            .arg("--card")
            .arg(f.path("card"))
            .arg("--state")
            .arg(f.path("state"));
        c
    };
    refused(&install().output().expect("test setup"), "running image unit 'none'");
    refused(
        &install().args(["--unit", "another-unit"]).output().expect("test setup"),
        "running image unit 'another-unit'",
    );
    let out = install().args(["--unit", "fake-unit", "--execute"]).output().expect("test setup");
    success(&out);
    let log = String::from_utf8(out.stdout).expect("test setup");
    assert!(!log.contains("override"));
    let mut c = card::Card::open(&f.path("card"), false).expect("test setup");
    assert_eq!(c.boot_magic().expect("test setup"), *b"OOSK");
}

#[test]
fn device_aliases_never_accept_a_simulated_unit_or_extra_keys() {
    let f = Fixture::new();
    std::os::unix::fs::symlink("/dev/null", f.path("card-link")).expect("device alias");
    for path in [PathBuf::from("/dev/null"), PathBuf::from("/dev/./null"), f.path("card-link")] {
        let out = f
            .command()
            .arg("install")
            .arg(f.path("image"))
            .arg("--state")
            .arg(f.path("state"))
            .args(["--unit", "fake-unit", "--card"])
            .arg(&path)
            .output()
            .expect("CLI runs");
        refused(&out, "regular test file");
        for extra_keys in [false, true] {
            let mut command = f.command();
            command.arg("gel-install").arg(f.path("image")).arg("--save").arg(f.path("save")).arg("--card").arg(&path);
            if extra_keys {
                command.arg("--keys").arg(f.path("public"));
            }
            refused(&command.output().expect("CLI runs"), "regular test file");
        }
    }
}

#[test]
fn regular_card_file_aliases_keep_simulation_and_unit_checks() {
    let f = Fixture::new();
    simulated_card(&f);
    std::fs::write(f.path("image"), image_unit(Some("fake-unit"))).expect("test setup");
    success(&f.test_pack().output().expect("test setup"));
    std::os::unix::fs::symlink(f.path("card"), f.path("card-link")).expect("file alias");
    for path in [f.path("./card"), f.path("card-link")] {
        let install = || {
            let mut c = f.command();
            c.arg("install")
                .arg(f.path("package"))
                .arg("--keys")
                .arg(f.path("test public key"))
                .arg("--card")
                .arg(&path)
                .arg("--state")
                .arg(f.path("state"));
            c
        };
        refused(&install().output().expect("CLI runs"), "running image unit 'none'");
        success(&install().args(["--unit", "fake-unit"]).output().expect("CLI runs"));
    }
}

#[test]
fn direct_cli_text_distinguishes_dry_run_installed_and_unchanged() {
    let f = Fixture::new();
    simulated_card(&f);
    success(&f.test_pack().output().expect("test setup"));
    let install = || {
        let mut c = f.command();
        c.arg("install")
            .arg(f.path("package"))
            .arg("--keys")
            .arg(f.path("test public key"))
            .arg("--card")
            .arg(f.path("card"))
            .arg("--state")
            .arg(f.path("state"));
        c
    };
    let dry = install().output().expect("dry run");
    success(&dry);
    let dry = String::from_utf8(dry.stdout).unwrap();
    assert!(dry.contains("dry run: nothing written"));
    assert!(!dry.contains("No automatic restart"));
    let installed = install().arg("--execute").output().expect("install");
    success(&installed);
    let installed = String::from_utf8(installed.stdout).unwrap();
    assert!(installed.contains("installed and armed (OOSK); its next ordinary start is the trial."));
    assert!(
        installed.contains("The trial confirms automatically after the UI runs 20 s continuously and startup finishes")
    );
    assert!(installed.contains("No automatic restart: power-cycle the scope to start the installed build on trial."));
    let unchanged = install().arg("--execute").output().expect("unchanged");
    success(&unchanged);
    let unchanged = String::from_utf8(unchanged.stdout).unwrap();
    assert!(unchanged.contains("nothing to do"));
    assert!(!unchanged.contains("No automatic restart"));
}
