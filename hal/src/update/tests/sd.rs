// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::update::sd::{self, Mode};

mod preference;
mod trial_evidence;

fn base_tree() -> Vec<u8> {
    dtb(
        &[("model", b"test\0")],
        vec![
            chosen(b"console=tty0\0"),
            fdt::Node {
                name: "mmc@fe320000".into(),
                props: vec![
                    ("cap-sd-highspeed".into(), vec![]),
                    ("max-frequency".into(), 150_000_000u32.to_be_bytes().to_vec()),
                    ("vqmmc-supply".into(), 7u32.to_be_bytes().to_vec()),
                ],
                kids: vec![],
            },
            fdt::Node {
                name: "LDO_REG4".into(),
                props: vec![
                    ("regulator-name".into(), b"vccio_sd\0".to_vec()),
                    ("phandle".into(), 7u32.to_be_bytes().to_vec()),
                    ("regulator-min-microvolt".into(), 1_800_000u32.to_be_bytes().to_vec()),
                    ("regulator-max-microvolt".into(), 3_300_000u32.to_be_bytes().to_vec()),
                ],
                kids: vec![],
            },
        ],
    )
}

fn active(name: &str) -> Fake {
    let f = Fake::new(name, None);
    let image = bootimg_second(1, rsce(&base_tree()));
    f.poke(RECOVERY_LBA, &image);
    State {
        current: Some(Entry { build: KX_BUILD.into(), sha256: hash::sha256(&image), len: image.len() as u64 }),
        ..State::default()
    }
    .save(&f.state())
    .unwrap();
    f
}

fn set_mode(f: &Fake, mode: &str) {
    std::fs::write(f.dir.join("oos/system.conf"), format!("# Device\nsd.mode = {}\n", mode)).unwrap();
}

/// A signed install applies SDR104. GEL then puts this package's default image back in recovery.
fn gel_rewrite(name: &str, stamp: &str) -> (Fake, Vec<u8>) {
    let f = Fake::new(name, Some(1));
    std::fs::create_dir(f.dir.join("oos")).unwrap();
    set_mode(&f, "sdr104");
    let mut tree = fdt::Fdt::parse(&base_tree()).unwrap();
    // Fill the last page of the default image so the SDR104 properties add a page.
    tree.root.set("padding", &[]);
    let padding = (1024 - tree.to_bytes().len()) / 4 * 4;
    tree.root.set("padding", &vec![0; padding]);
    let stamped = bootimg_build(2, stamp);
    let ramdisk = kexec::parts(&stamped).unwrap().1.to_vec();
    let default = bootimg_parts(2, ramdisk, rsce(&tree.to_bytes()));
    let package = pkg::build(&SEED, KX_BUILD, "test", &default).unwrap();
    assert!(install_direct(&mut f.card(), &f.state(), &package, &[pk()], &RUN).unwrap().written);
    let uhs = f.region(Region::Recovery);
    assert_ne!(check_image(&default).unwrap().id, check_image(&uhs).unwrap().id);
    assert_ne!(default.len(), uhs.len());
    (f, default)
}

#[test]
fn gel_reinstall_retracks_the_default_tree_and_keeps_rollback() {
    let (f, default) = gel_rewrite("gel-same-build", KX_BUILD);
    assert_eq!(boot_started(&f.state(), Some(KX_BUILD)).unwrap(), Boot::Trial(1));
    assert!(confirm_running(&f.state(), None, Some(KX_BUILD)).unwrap());
    let mut expected = f.st();
    f.poke(RECOVERY_LBA, &default);

    let mut card = f.card();
    let reason = reconcile_recovery(&mut card, &f.state()).unwrap().unwrap();
    assert_eq!(reason, format!("recovery image for build {} was rewritten by an installer", short(KX_BUILD)));
    let entry = Entry { build: KX_BUILD.into(), sha256: hash::sha256(&default), len: default.len() as u64 };
    expected.track_recovery(entry, check_image(&default).unwrap().id);
    expected.event = reason;
    assert_eq!(f.st(), expected);
    assert_eq!(f.region(Region::Recovery), default);
    assert_eq!(f.region(Region::Slot), bootimg(1));
    assert_eq!(&f.magic(), b"OOSK");
    assert_eq!(reconcile_recovery(&mut card, &f.state()).unwrap(), None);
    assert_eq!(f.st(), expected);
}

#[test]
fn gel_reinstall_with_a_different_image_stamp_ends_tracking() {
    // Even an image that reproduces the saved SDR104 ID must name the tracked build in its own initramfs.
    let (f, default) = gel_rewrite("gel-different-stamp", "20261101-000000 kernel fixture");
    assert_eq!(boot_started(&f.state(), Some(KX_BUILD)).unwrap(), Boot::Trial(1));
    assert!(confirm_running(&f.state(), None, Some(KX_BUILD)).unwrap());
    f.poke(RECOVERY_LBA, &default);

    let reason = reconcile_recovery(&mut f.card(), &f.state()).unwrap().unwrap();
    assert!(reason.starts_with("recovery changed since build "), "{reason}");
    let state = f.st();
    assert_eq!(state.untracked_reason(), Some(reason.as_str()));
    assert!(state.current.is_none() && state.previous.is_none());
    assert_eq!(f.region(Region::Slot), bootimg(1));
}

#[test]
fn gel_reinstall_during_a_trial_leaves_the_state_alone() {
    let (f, default) = gel_rewrite("gel-open-trial", KX_BUILD);
    let pending = f.st();
    assert_eq!(pending.trial, Some(0));
    f.poke(RECOVERY_LBA, &default);

    let mut card = f.card();
    assert_eq!(reconcile_recovery(&mut card, &f.state()).unwrap(), None);
    assert_eq!(card.read_bytes, 0);
    assert_eq!(f.st(), pending);
    assert_eq!(f.region(Region::Recovery), default);
    assert_eq!(f.region(Region::Slot), bootimg(1));
}

#[test]
fn unchanged_tree_merge_failure_explains_why_to_install_directly() {
    let f = active("same-tree-phandle");
    let booted = rsce(&base_tree());
    let odd = dtb(&[("model", b"test\0"), ("phandle", &[0, 0, 0, 9])], vec![chosen(b"console=tty0\0")]);
    let candidate = bootimg_second(2, booted.clone());
    let package = pkg::build_with(&SEED, KX_BUILD, "test", &candidate, &[kexec::LINE]).unwrap();
    let trees = BootTrees { booted: Some(&booted), running_build: Some(KX_BUILD), live: Some(&odd) };
    let before = f.st();
    let error = kexec_stage(&mut f.card(), &f.state(), &package, &[pk()], &RUN, trees).unwrap_err();
    for expected in ["unchanged device tree", "phandle", "kexec_file_load", "low-memory reservations", "--new-dt"] {
        assert!(error.contains(expected), "{error}");
    }
    assert_eq!(f.st(), before);
    assert_eq!(f.region(Region::Recovery), bootimg_second(1, booted));
    assert_eq!(&f.magic(), b"KRNL");
}

#[test]
fn absent_mode_is_hs_and_invalid_modes_are_refused() {
    assert_eq!(Mode::default(), Mode::Hs);
    assert_eq!(Mode::parse("sd.mode = invalid\nsd.mode = hs\n").unwrap(), Mode::Hs);
    assert_eq!(Mode::parse("# sd.mode = sdr104\nntp = on\n").unwrap(), Mode::Hs);
    assert_eq!(Mode::parse("sd.mode = hs\n sd.mode = sdr104 # card\n").unwrap(), Mode::Sdr104);
    for value in ["", "SDR104", "sdr50", "invalid"] {
        assert!(Mode::parse(&format!("sd.mode = {}", value)).is_err());
    }
    let f = active("sd-default");
    let before = f.region(Region::Recovery);
    assert!(sd::restage(&mut f.card(), &f.state(), true).unwrap().unchanged);
    assert_eq!(f.region(Region::Recovery), before);
    assert!(f.st().trial.is_none());
}

#[test]
fn every_uhs_property_reads_back_with_fdtget() {
    let f = active("sd-readback");
    let mut base = fdt::Fdt::parse(&base_tree()).unwrap();
    base.root.kids[2].props.push(("regulator-init-microvolt".into(), 3_300_000u32.to_be_bytes().to_vec()));
    let tree = sd::patch_tree(&base.to_bytes(), Mode::Sdr104).unwrap();
    let file = f.dir.join("tree.dtb");
    std::fs::write(&file, &tree).unwrap();
    let get = |node: &str, key: &str| {
        std::process::Command::new("fdtget")
            .arg("-t")
            .arg("u")
            .arg(&file)
            .args([node, key])
            .output()
            .expect("host DTB tests require fdtget")
    };
    for (node, key, value) in [
        ("/mmc@fe320000", "sd-uhs-sdr50", ""),
        ("/mmc@fe320000", "sd-uhs-sdr104", ""),
        ("/mmc@fe320000", "max-frequency", "150000000"),
        ("/LDO_REG4", "regulator-settling-time-down-us", "30000"),
        ("/LDO_REG4", "regulator-min-microvolt", "1800000"),
        ("/LDO_REG4", "regulator-max-microvolt", "3300000"),
        ("/mmc@fe320000", "vqmmc-supply", "7"),
    ] {
        let out = get(node, key);
        assert!(out.status.success(), "{}: {}", key, String::from_utf8_lossy(&out.stderr));
        assert_eq!(String::from_utf8(out.stdout).unwrap().trim(), value, "{}", key);
    }
    assert!(!get("/LDO_REG4", "regulator-init-microvolt").status.success());
    assert_eq!(sd::patch_tree(&tree, Mode::Sdr104).unwrap(), tree);
    assert_eq!(sd::patch_tree(&sd::patch_tree(&base_tree(), Mode::Sdr104).unwrap(), Mode::Hs).unwrap(), base_tree());
}

#[test]
fn restaging_is_a_next_boot_trial_with_verified_hs_rollback() {
    let f = active("sd-trial");
    let hs = f.region(Region::Recovery);
    let before = card_bytes(&f);
    set_mode(&f, "sdr104");
    assert!(!sd::restage(&mut f.card(), &f.state(), false).unwrap().written);
    assert_eq!(card_bytes(&f), before);
    assert!(sd::restage(&mut f.card(), &f.state(), true).unwrap().written);
    let uhs = f.region(Region::Recovery);
    assert_eq!(uhs, sd::image(&hs, Mode::Sdr104).unwrap());
    card::check_image(&uhs).unwrap();
    assert_eq!(f.region(Region::Slot), hs);
    assert_eq!(f.st().current.unwrap().sha256, hash::sha256(&uhs));
    assert_eq!(f.st().trial, Some(0));
    assert!(confirm_running(&f.state(), None, Some(KX_BUILD)).unwrap_err().contains("not booted"));
    assert!(sd::restage(&mut f.card(), &f.state(), true).is_err());
    assert_eq!(
        boot_started_with_card(&f.state(), Some(KX_BUILD), trial_evidence::observation(&f, "hs", "sdr104", false))
            .unwrap(),
        Boot::Trial(1)
    );
    assert_eq!(boot_started(&f.state(), Some(KX_BUILD)).unwrap(), Boot::Rollback);
    let r = rollback(&mut f.card(), &f.state(), "unconfirmed SD mode", true, None).unwrap();
    assert!(r.lines.iter().any(|l| l.contains("SDR104 fallback")));
    assert_eq!(f.region(Region::Recovery), hs);
    assert!(f.st().sd.failed);
    assert!(sd::restage(&mut f.card(), &f.state(), true).unwrap().unchanged);
    set_mode(&f, "hs");
    sd::restage(&mut f.card(), &f.state(), true).unwrap();
    assert!(f.st().sd.failed);
    set_mode(&f, "sdr104");
    assert!(sd::restage_preference(&mut f.card(), &f.state(), Some(Mode::Sdr104), true).unwrap().written);
    assert!(!f.st().sd.failed);
}

#[test]
fn a_confirmed_sd_trial_survives_and_switching_back_to_hs_works() {
    let f = active("sd-confirm");
    let hs = f.region(Region::Recovery);
    set_mode(&f, "sdr104");
    sd::restage(&mut f.card(), &f.state(), true).unwrap();
    boot_started(&f.state(), Some(KX_BUILD)).unwrap();
    let sha = f.st().current.unwrap().sha256;
    assert!(confirm_with_sd_mode(&f.state(), sha, crate::bootcard::Mode::Sdr104).unwrap());
    assert!(f.st().event.ends_with(" (SDR104 trial, observed sdr104)"));
    assert!(sd::restage(&mut f.card(), &f.state(), true).unwrap().unchanged);
    set_mode(&f, "hs");
    assert!(sd::restage(&mut f.card(), &f.state(), true).unwrap().written);
    assert_eq!(f.region(Region::Recovery), hs);
}

#[test]
fn a_second_consecutive_missed_sdr104_handoff_selects_hs_for_install() {
    let f = active("sd-install");
    let hs = f.region(Region::Recovery);
    set_mode(&f, "sdr104");
    let candidate = bootimg_second(2, rsce(&base_tree()));
    let package = pkg::build_with(&SEED, "20261109-090000", "test", &candidate, &[kexec::LINE]).unwrap();
    let second = kexec::parts(&hs).unwrap().2.to_vec();
    for (count, mode) in [(1, Mode::Sdr104), (2, Mode::Hs)] {
        let (_, staged) = kexec_stage(
            &mut f.card(),
            &f.state(),
            &package,
            &[pk()],
            &RUN,
            BootTrees { booted: Some(&second), running_build: Some(KX_BUILD), live: Some(&base_tree()) },
        )
        .unwrap();
        assert_eq!(staged.unwrap().tree.unwrap(), sd::patch_tree(&base_tree(), Mode::Sdr104).unwrap());
        assert_eq!(f.region(Region::Recovery), hs, "kexec keeps the installed HS fallback untouched");
        let missed = kexec_missed(&f.state(), false).unwrap().unwrap();
        assert!(missed.contains("not installed"));
        assert!(missed.contains("The card mode may be the cause. See System > Boot card mode."));
        assert_eq!(kexec_missed(&f.state(), false).unwrap(), None);
        assert_eq!(f.st().sd.trial, None);
        assert_eq!(f.st().sd.missed, count);
        assert_eq!(sd::configured(&f.state(), &f.st()).unwrap(), mode);
    }
    let mut damaged = package.clone();
    *damaged.last_mut().unwrap() ^= 1;
    let before = card_bytes(&f);
    assert!(install(&mut f.card(), &f.state(), &damaged, &[pk()], &RUN).is_err());
    assert_eq!(card_bytes(&f), before);
    install(&mut f.card(), &f.state(), &package, &[pk()], &RUN).unwrap();
    assert_eq!(f.region(Region::Recovery), candidate);
    assert_eq!(f.region(Region::Slot), hs);
}

#[test]
fn restaging_records_the_verified_rollback_before_disarming_oos() {
    let f = active("sd-disarm-failure");
    let current = f.st().current;
    let hs = f.region(Region::Recovery);
    set_mode(&f, "sdr104");
    f.poke(BOOT_LBA, b"FAIL");

    let error = sd::restage(&mut f.card(), &f.state(), true).unwrap_err();
    assert!(error.contains("neither KRNL nor OOSK"), "{error}");
    assert_eq!(f.region(Region::Slot), hs);
    let state = f.st();
    assert_eq!(state.previous, current);
    assert_eq!(state.current, current);
    assert_eq!(state.trial, None);
    assert_eq!(f.region(Region::Recovery), hs);
    assert_eq!(&f.magic(), b"FAIL");
}

#[test]
fn a_cut_sd_write_keeps_stock_selected_and_hs_in_the_slot() {
    let f = active("sd-cut");
    let hs = f.region(Region::Recovery);
    set_mode(&f, "sdr104");
    let mut card = f.card();
    card.cut = Some(Region::Recovery);
    assert!(sd::restage(&mut card, &f.state(), true).is_err());
    assert_eq!(&f.magic(), b"KRNL");
    assert_eq!(f.region(Region::Slot), hs);
    rollback(&mut f.card(), &f.state(), "cut write", true, None).unwrap();
    assert_eq!(f.region(Region::Recovery), hs);
}

#[test]
fn restaging_refuses_unknown_slot_contents_before_any_write() {
    let f = active("sd-foreign-slot");
    set_mode(&f, "sdr104");
    f.poke(SLOT_LBA, b"unowned data");
    let before = card_bytes(&f);
    let error = sd::restage(&mut f.card(), &f.state(), true).unwrap_err();
    assert!(error.contains("holds data that is not an OOS image"));
    assert_eq!(card_bytes(&f), before);
    assert!(f.st().trial.is_none());
}

#[test]
fn a_staged_sdr104_start_keeps_the_signed_slot_bytes_and_uses_the_selected_tree() {
    let f = active("sd-slot-staging");
    let hs = f.region(Region::Recovery);
    let candidate = bootimg_second(2, rsce(&base_tree()));
    set_mode(&f, "sdr104");
    let build = "20261109-090000";
    let package = pkg::build_with(
        &SEED,
        build,
        "test",
        &candidate,
        &[kexec::LINE, kexec::STAGED_LINE, (kexec::STAGED_KEY, &hex(&pk()))],
    )
    .unwrap();
    let (_, _, booted) = kexec::parts(&hs).unwrap();
    let (_, start) = kexec_stage(
        &mut f.card(),
        &f.state(),
        &package,
        &[pk()],
        &RUN,
        BootTrees { booted: Some(booted), running_build: Some(KX_BUILD), live: Some(&base_tree()) },
    )
    .unwrap();
    let start = start.unwrap();
    let tree = start.tree.as_ref().unwrap();
    assert_eq!(tree, &sd::patch_tree(&base_tree(), Mode::Sdr104).unwrap());
    assert_eq!(start.payload_size(), start.kernel.len() + start.initrd.len() + tree.len());
    assert_eq!(f.region(Region::Slot), candidate);
    assert_eq!(f.region(Region::Recovery), hs);

    let header = f.dir.join("header");
    let (_, ramdisk, _) = kexec::parts(&candidate).unwrap();
    let archive = start.initrd.strip_prefix(ramdisk).unwrap();
    std::fs::write(&header, inflate::cpio_file(archive, "oos-kexec.hdr").unwrap()).unwrap();
    assert!(kexec::install_staged(&mut f.card(), &f.state(), &header, build, &[pk()], &RUN).unwrap().written);
    assert_eq!(f.region(Region::Slot), hs);
    assert_eq!(f.region(Region::Recovery), sd::image(&candidate, Mode::Sdr104).unwrap());
}
