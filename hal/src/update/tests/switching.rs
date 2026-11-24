// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

const TRACKED: &str = "20261124-130000";
const OLDER: &str = "20261123-130000";
const HEADER_AND_LAYOUT: usize = 2048 + 1024;

fn tracked_card(name: &str) -> Fake {
    let f = Fake::new(name, Some(1));
    assert!(install_direct(&mut f.card(), &f.state(), &package(2, TRACKED), &[pk()], &RUN).unwrap().written);
    assert!(confirm_running(&f.state(), None, Some(TRACKED)).unwrap());
    f
}

#[test]
fn only_untracked_recovery_clears_the_downgrade_base() {
    let older_package = package(4, OLDER);
    for (name, replacement) in [
        ("foreign", Some(bootimg_build(3, "foreign-build"))),
        ("garbage", Some(vec![0x55; bootimg(2).len()])),
        ("older", Some(bootimg_build(3, OLDER))),
        ("unchanged", None),
    ] {
        let f = tracked_card(name);
        let mut card = f.card();
        card.disarm_oos().unwrap();
        if let Some(bytes) = &replacement {
            f.poke(RECOVERY_LBA, bytes);
        }
        let before = f.st();
        let recovery = card.read(RECOVERY_LBA, bootimg(2).len()).unwrap();
        let reason = reconcile_recovery(&mut card, &f.state()).unwrap();
        assert_eq!(card.read(RECOVERY_LBA, recovery.len()).unwrap(), recovery);
        assert_eq!(f.region(Region::Slot), bootimg(1));
        assert_eq!(&f.magic(), b"KRNL");
        assert_eq!(boot_started(&f.state(), Some(OLDER)).unwrap(), Boot::NoTrial);
        if let Some(replacement) = replacement {
            let reason = reason.expect("a mismatched recovery ends tracking");
            if name == "garbage" {
                assert!(reason.contains("no recognizable OOS image"));
            } else {
                assert!(reason.contains(&hex(&hash::sha256(&replacement))));
            }
            assert_untracked(&f, before, reason);
            assert_eq!(reconcile_recovery(&mut card, &f.state()).unwrap(), None);
            assert!(install_direct(&mut card, &f.state(), &older_package, &[pk()], &RUN).unwrap().written);
            assert_eq!(f.region(Region::Recovery), bootimg(4));
            assert_eq!(f.st().untracked_reason(), None);
        } else {
            assert_eq!(reason, None);
            assert_eq!(f.st(), before);
            let error = install_direct(&mut card, &f.state(), &older_package, &[pk()], &RUN).unwrap_err();
            assert!(error.contains("older than the installed"), "{error}");
            assert_eq!(f.region(Region::Recovery), bootimg(2));
        }
    }
}

#[test]
fn a_matching_recovery_reads_only_the_header_and_layout() {
    let f = tracked_card("reconcile-header-only");
    let mut before = f.st();
    for legacy_state in [false, true] {
        if legacy_state {
            before.recovery.loader_id = None;
            before.save(&f.state()).unwrap();
            let mut card = f.card();
            assert_eq!(reconcile_recovery(&mut card, &f.state()).unwrap(), None);
            assert_eq!(card.read_bytes, HEADER_AND_LAYOUT + bootimg(2).len());
            assert_eq!(f.st().recovery.loader_id, Some(image_header(&bootimg(2)).unwrap().id));
        }
        let mut card = f.card();
        let state = f.st();
        assert_eq!(reconcile_recovery(&mut card, &f.state()).unwrap(), None);
        assert_eq!(card.read_bytes, HEADER_AND_LAYOUT);
        assert_eq!(f.st(), state);
    }
}

#[test]
fn start_completes_a_rollback_cut_after_recovery_readback() {
    for same_loader_id in [false, true] {
        let f = Fake::new("reconcile-cut-rollback", None);
        let mut card = f.card();
        let previous = bootimg(1);
        let mut current = bootimg(if same_loader_id { 1 } else { 2 });
        // Padding is outside the loader hash, but the state records the hash of the whole image.
        *current.last_mut().unwrap() = 1;
        for (build, image) in [(OLDER, &previous), (TRACKED, &current)] {
            let package = pkg::build(&SEED, build, "test", image).unwrap();
            assert!(install_direct(&mut card, &f.state(), &package, &[pk()], &RUN).unwrap().written);
            assert!(confirm_running(&f.state(), None, Some(build)).unwrap());
        }
        card.write_fault = Some((Region::Recovery, WriteFault::CutAfterReadBack));
        assert_eq!(rollback(&mut card, &f.state(), "manual", true, None).unwrap_err(), "power cut after read-back");
        let cut = f.st();
        assert_eq!(cut.current.as_ref().unwrap().build, TRACKED);
        assert_eq!(cut.previous.as_ref().unwrap().build, OLDER);
        assert_eq!(f.region(Region::Recovery), previous);
        assert_eq!(&f.magic(), b"KRNL");

        let reason = reconcile_recovery(&mut card, &f.state()).unwrap().unwrap();
        assert!(reason.contains("completed rollback"), "{reason}");
        assert_eq!(boot_started(&f.state(), Some(OLDER)).unwrap(), Boot::NoTrial);
        let state = f.st();
        assert_eq!(state.current, cut.previous);
        assert_eq!(state.previous, cut.previous);
        assert_eq!(state.untracked_reason(), None);
        assert_eq!(f.region(Region::Slot), previous);
        assert_eq!(&f.magic(), b"KRNL");
        let downgrade = package(3, "20261122-130000");
        assert!(install_direct(&mut card, &f.state(), &downgrade, &[pk()], &RUN)
            .unwrap_err()
            .contains("older than the installed"));
    }
}

fn assert_untracked(f: &Fake, before: State, reason: String) {
    let recovery = RecoveryTracking { untracked: Some(reason), ..Default::default() };
    assert_eq!(f.st(), State { current: None, previous: None, recovery, ..before });
}

#[test]
fn reconciliation_and_leave_preserve_open_trials_and_candidates() {
    for (trial, staged) in [(Some(1), None), (None, Some(Entry { build: OLDER.into(), sha256: [1; 32], len: 2048 }))] {
        let f = tracked_card("switch-pending");
        let mut before = f.st();
        before.trial = trial;
        before.staged = staged;
        before.save(&f.state()).unwrap();
        let before = f.st();
        f.poke(RECOVERY_LBA, &bootimg(3));
        assert_eq!(reconcile_recovery(&mut f.card(), &f.state()).unwrap(), None);
        assert!(leave(&f.state()).is_err());
        assert_eq!(f.st(), before);
        assert_eq!(f.region(Region::Recovery), bootimg(3));
        assert_eq!(f.region(Region::Slot), bootimg(1));
        assert_eq!(&f.magic(), b"OOSK");
    }
}

#[test]
fn leave_only_clears_tracking_and_reports_the_existing_boot_mode() {
    for persist in [false, true] {
        let f = tracked_card("switch-leave");
        let mut before = f.st();
        before.persist = persist;
        before.sd_failed = true;
        before.slot_incomplete = true;
        before.save(&f.state()).unwrap();
        assert_eq!(leave(&f.state()).unwrap(), persist);
        let after = f.st();
        assert_eq!(after.untracked_reason(), Some("tracking cleared by oos-update leave"));
        assert_untracked(&f, before, "tracking cleared by oos-update leave".into());
        assert_eq!(&f.magic(), b"OOSK");
        assert_eq!(f.region(Region::Recovery), bootimg(2));
        assert_eq!(f.region(Region::Slot), bootimg(1));
    }
}

#[test]
fn switching_back_receives_a_legacy_bridge_then_returns_to_our_own_key() {
    let f = tracked_card("cross-install");
    let mut card = f.card();
    let other_seed = [65; 32];
    let theirs = ed25519::public_key(&other_seed);
    let foreign_image = bootimg_dt(3, 7);
    let bridge_image = bootimg_dt(4, 7);
    let model = "DHO924S";
    let opt = InstallOpt { running_model: model, ..RUN };
    let foreign = pkg::build_with(&other_seed, "20261125-130000", "test", &foreign_image, &[kexec::LINE]).unwrap();
    let bridge = pkg::build_with(&other_seed, OLDER, "test", &bridge_image, &[("models", model), kexec::LINE]).unwrap();
    let next = pkg::build_with(&SEED, TRACKED, "test", &bootimg(5), &[("models", model)]).unwrap();

    let confirmed = f.st();
    assert_eq!(confirmed.current.as_ref().unwrap().build, TRACKED);
    assert_eq!(confirmed.trial, None);
    assert_eq!(&f.magic(), b"OOSK");
    assert_eq!(f.region(Region::Recovery), bootimg(2));
    assert_eq!(f.region(Region::Slot), bootimg(1));

    externally_install(&f, &mut card, &foreign, theirs);

    card.disarm_oos().unwrap();
    let reason = reconcile_recovery(&mut card, &f.state()).unwrap().unwrap();
    assert_untracked(&f, confirmed, reason);
    assert_eq!(&f.magic(), b"KRNL");
    assert_eq!(f.region(Region::Recovery), foreign_image);
    assert_eq!(f.region(Region::Slot), bootimg(1));

    receive_bridge(&f, &mut card, &bridge, theirs, &opt);
    assert_eq!(f.region(Region::Recovery), bridge_image);
    assert_eq!(f.region(Region::Slot), foreign_image);
    assert_eq!(&f.magic(), b"OOSK");
    let on_trial = f.st();
    assert_eq!(on_trial.current.as_ref().unwrap().build, OLDER);
    assert_eq!(on_trial.previous.as_ref().unwrap().sha256, hash::sha256(&foreign_image));
    assert_eq!(on_trial.kexec, None);
    assert_eq!(on_trial.trial, Some(0));
    assert_eq!(on_trial.untracked_reason(), None);
    assert!(confirm_running(&f.state(), None, Some(OLDER)).unwrap());

    assert!(pkg::verify(&next, &[theirs]).unwrap_err().contains("unknown key"));
    assert_release_key_only(&f, &mut card, &bridge, &next, &opt);
}

fn externally_install(f: &Fake, card: &mut Card, package: &[u8], signer: [u8; 32]) {
    let before = f.st();
    let slot = f.region(Region::Slot);
    assert!(pkg::verify(package, &[pk()]).unwrap_err().contains("unknown key"));
    let (header, verified) = pkg::verify(package, &[signer]).unwrap();
    assert_eq!(header.manifest.extra, vec![(kexec::LINE.0.into(), kexec::LINE.1.into())]);
    card.disarm_oos().unwrap();
    card.write_image(Region::Recovery, verified).unwrap();
    card.arm_oos().unwrap();
    assert_eq!(f.region(Region::Recovery), verified);
    assert_eq!(f.region(Region::Slot), slot);
    assert_eq!(&f.magic(), b"OOSK");
    assert_eq!(f.st(), before);
}

fn assert_release_key_only(f: &Fake, card: &mut Card, bridge: &[u8], next: &[u8], opt: &InstallOpt<'_>) {
    let before = f.st();
    let recovery = f.region(Region::Recovery);
    let slot = f.region(Region::Slot);
    assert!(install_direct(card, &f.state(), bridge, &[pk()], opt).unwrap_err().contains("unknown key"));
    assert_eq!(f.st(), before);
    assert_eq!(f.region(Region::Recovery), recovery);
    assert_eq!(f.region(Region::Slot), slot);
    assert!(install_direct(card, &f.state(), next, &[pk()], opt).unwrap().written);
    assert_eq!(f.region(Region::Recovery), bootimg(5));
    assert_eq!(f.region(Region::Slot), recovery);
    assert_eq!(f.st().previous.unwrap().build, OLDER);
    assert_eq!(&f.magic(), b"OOSK");
    assert_eq!(card.read(BACKUP_LBA, 2048).unwrap(), vec![0; 2048]);
    assert_eq!(card.read(OOSDATA_LBA, 2048).unwrap(), vec![0; 2048]);
}

fn receive_bridge(f: &Fake, card: &mut Card, bridge: &[u8], signer: [u8; 32], opt: &InstallOpt<'_>) {
    let recovery = f.region(Region::Recovery);
    let slot = f.region(Region::Slot);
    let trees = BootTrees { booted: Some(&tree(7)), running_build: None, live: None };
    let (_, start) = kexec_stage(card, &f.state(), bridge, &[signer], opt, trees).unwrap();
    let start = start.unwrap();
    assert_eq!(kexec::receive(&start.cmdline("quiet")), Some(kexec::Receive::Legacy(kexec::Flags::default())));
    assert_eq!(f.st().kexec.as_deref(), Some(OLDER));
    assert!(f.st().untracked_reason().is_some());
    assert_eq!(f.region(Region::Recovery), recovery);
    assert_eq!(f.region(Region::Slot), slot);
    assert_eq!(&f.magic(), b"KRNL");
    let (_, image) = pkg::verify(bridge, &[signer]).unwrap();
    let (_, ramdisk, _) = kexec::parts(image).unwrap();
    let archive = start.initrd.strip_prefix(ramdisk).unwrap();
    let file = |name: &str| inflate::cpio_file(archive, name.trim_start_matches('/')).unwrap();
    let carried_key = pkg::parse_keys(std::str::from_utf8(file(kexec::KEY_FILE)).unwrap());
    assert_eq!(carried_key, [signer]);
    let received = file(kexec::PACKAGE);
    assert_eq!(received, bridge);

    // The legacy receiver trusts the first stage's carrying key for this install only.
    assert!(install(card, &f.state(), received, &[pk(), carried_key[0]], opt).unwrap().written);
}
