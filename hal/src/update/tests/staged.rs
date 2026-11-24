// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

mod sender;

const OLD: &str = "20261105-160000 kernel x";
const NEW: &str = "20261105-170000 kernel x";

fn entry(tag: u8, build: &str) -> Entry {
    let image = bootimg(tag);
    Entry { build: build.into(), sha256: hash::sha256(&image), len: image.len() as u64 }
}

fn card_with_staged_image(name: &str) -> Fake {
    let f = Fake::new(name, Some(1));
    f.poke(SLOT_LBA, &bootimg(2));
    let state = State { current: Some(entry(1, OLD)), kexec: Some(NEW.into()), ..State::default() };
    state.save(&f.state()).unwrap();
    f
}

fn header(tag: u8, build: &str, announce: bool) -> Vec<u8> {
    let lines = [kexec::LINE, kexec::STAGED_LINE];
    let mut package =
        pkg::build_with(&SEED, build, "6.12.115+", &bootimg(tag), &lines[..1 + usize::from(announce)]).unwrap();
    package.truncate(pkg::verify_header(&package, &[pk()]).unwrap().data_offset);
    package
}

fn receive(f: &Fake, card: &mut Card, header_file: &Path) -> Result<Report, kexec::StagedError> {
    kexec::install_staged(card, &f.state(), header_file, NEW, &[pk()], &RUN)
}

fn refusal(result: Result<Report, kexec::StagedError>) -> String {
    match result.unwrap_err() {
        kexec::StagedError::Refused(reason) => reason,
        error => panic!("expected a refusal before card writes, got {error:?}"),
    }
}

fn signed_size_header(size: usize) -> Vec<u8> {
    let mut manifest = pkg::verify_header(&header(2, NEW, true), &[pk()]).unwrap().manifest;
    manifest.size = size as u64;
    let text = manifest.signed_text();
    let signature = ed25519::sign(&SEED, text.as_bytes());
    format!("{text}sig {}\n\n", hex(&signature)).into_bytes()
}

fn valid_header(f: &Fake) -> PathBuf {
    let path = f.dir.join("header");
    std::fs::write(&path, header(2, NEW, true)).unwrap();
    path
}

#[test]
fn staged_receive_installs_and_sets_the_legacy_trial_state() {
    let staged = card_with_staged_image("staged-install");
    let legacy = Fake::new("staged-legacy", Some(1));
    staged.st().save(&legacy.state()).unwrap();

    assert!(receive(&staged, &mut staged.card(), &valid_header(&staged)).unwrap().written);
    assert!(install(&mut legacy.card(), &legacy.state(), &package(2, NEW), &[pk()], &RUN).unwrap().written);
    assert_eq!(staged.region(Region::Recovery), bootimg(2));
    assert_eq!(staged.region(Region::Slot), bootimg(1));
    assert_eq!(&staged.magic(), b"OOSK");
    assert_eq!(staged.st(), legacy.st());
}

#[test]
fn staged_refusals_leave_card_and_install_state_untouched() {
    let valid = header(2, NEW, true);
    let mut bad_signature = valid.clone();
    let at = bad_signature.windows(4).position(|word| word == b"sig ").unwrap() + 4;
    bad_signature[at] = if bad_signature[at] == b'0' { b'1' } else { b'0' };
    let mut trailing = valid.clone();
    trailing.push(0);
    let wrong_build = header(2, OLD, true);
    let unannounced = header(2, NEW, false);
    let empty = signed_size_header(0);
    let oversized = signed_size_header(IMAGE_MAX + 1);

    let cases = [
        ("hash", Some(&valid), Some(vec![0; bootimg(2).len()]), false, "SHA-256 mismatch"),
        ("wrong-build", Some(&wrong_build), None, false, "does not match running build"),
        ("no-header", None, None, false, "reading the staged header"),
        ("short-slot", Some(&valid), None, true, "reading the staged image from the rollback slot"),
        ("stale-slot", Some(&valid), Some(bootimg(3)), false, "SHA-256 mismatch"),
        ("signature", Some(&bad_signature), None, false, "SIGNATURE INVALID"),
        ("unannounced", Some(&unannounced), None, false, "no 'trial staged' line"),
        ("trailing", Some(&trailing), None, false, "trailing bytes"),
        ("empty-image", Some(&empty), None, false, "staged image size 0 does not fit the rollback slot"),
        ("oversized-image", Some(&oversized), None, false, "does not fit the rollback slot"),
    ];
    for (name, header, slot, truncate_slot, reason) in cases {
        let f = card_with_staged_image(name);
        let path = f.dir.join("header");
        if let Some(header) = header {
            std::fs::write(&path, header).unwrap();
        }
        if let Some(slot) = slot {
            f.poke(SLOT_LBA, &slot);
        }
        let before = f.st();
        let slot_len = bootimg(2).len() - usize::from(truncate_slot);
        if truncate_slot {
            std::fs::OpenOptions::new()
                .write(true)
                .open(f.dir.join("card"))
                .unwrap()
                .set_len(SLOT_LBA * SECTOR + slot_len as u64)
                .unwrap();
        }
        let mut card = f.card();
        let original_slot = card.read(SLOT_LBA, slot_len).unwrap();
        let error = refusal(receive(&f, &mut card, &path));
        assert!(error.contains(reason), "{name}: {error}");
        assert_eq!(&f.magic(), b"KRNL", "{name}");
        assert_eq!(f.region(Region::Recovery), bootimg(1), "{name}");
        assert_eq!(card.read(SLOT_LBA, slot_len).unwrap(), original_slot, "{name}");
        assert_eq!(f.st(), before, "{name}");
        kexec::record_staged_refusal(&f.state(), &error).unwrap();
        assert_eq!(f.st(), State { kexec: None, event: error, ..before }, "{name}");
    }
}

#[test]
fn staged_receive_refuses_without_a_usable_recovery_image_for_rollback() {
    let mut damaged = bootimg(1);
    damaged[3000] ^= 1;
    assert!(check_image(&damaged).unwrap_err().contains("id mismatch"));
    let mut stock = vec![0; damaged.len()];
    stock[..4].copy_from_slice(b"KRNL");
    for (name, recovery) in [
        ("empty-recovery", vec![0; bootimg(1).len()]),
        ("stock-recovery", stock),
        ("damaged-recovery", damaged),
        ("identical-recovery", bootimg(2)),
    ] {
        let f = card_with_staged_image(name);
        f.poke(RECOVERY_LBA, &recovery);
        let mut before = f.st();
        before.previous = Some(entry(3, OLD));
        before.save(&f.state()).unwrap();
        let mut card = f.card();
        let opt = InstallOpt { force: true, ..RUN };
        let error = refusal(kexec::install_staged(&mut card, &f.state(), &valid_header(&f), NEW, &[pk()], &opt));
        assert!(error.contains("different, valid image in recovery"), "{name}: {error}");
        assert_eq!(card.read(RECOVERY_LBA, recovery.len()).unwrap(), recovery, "{name}");
        assert_eq!(f.region(Region::Slot), bootimg(2), "{name}");
        assert_eq!(&f.magic(), b"KRNL", "{name}");
        assert_eq!(f.st(), State { previous: None, slot_incomplete: true, ..before }, "{name}");
    }
}

#[test]
fn staged_build_and_kexec_build_have_the_same_line_encoding() {
    let build = "new build\ntrial 99";
    let state = State { staged: Some(entry(2, build)), kexec: Some(build.into()), ..State::default() };
    let after = State::parse(&state.text());
    assert_eq!(after.staged.unwrap().build, "new build trial 99");
    assert_eq!(after.kexec.as_deref(), Some("new build trial 99"));
    assert_eq!(after.trial, None);
}

#[test]
fn staged_receive_refuses_an_open_trial_or_a_model_mismatch() {
    for (name, trial, running_model, reason) in [
        ("staged-open-trial", Some(1), RUN.running_model, "wait for confirmation"),
        ("staged-model-mismatch", None, "DHO804", "running device model 'DHO804': refused"),
    ] {
        let f = card_with_staged_image(name);
        let before = State { trial, staged: Some(entry(2, NEW)), slot_incomplete: true, ..f.st() };
        before.save(&f.state()).unwrap();
        let opt = InstallOpt { running_model, ..RUN };
        let error = refusal(kexec::install_staged(&mut f.card(), &f.state(), &valid_header(&f), NEW, &[pk()], &opt));
        assert!(error.contains(reason), "{name}: {error}");
        assert_eq!(f.st(), before, "{name}");
        assert_eq!(f.region(Region::Recovery), bootimg(1), "{name}");
        assert_eq!(f.region(Region::Slot), bootimg(2), "{name}");
        assert_eq!(&f.magic(), b"KRNL", "{name}");
    }
}

#[test]
fn staged_refusal_records_its_reason_without_a_kexec_mark() {
    let f = Fake::new("staged-no-mark", Some(1));
    kexec::record_staged_refusal(&f.state(), "missing header").unwrap();
    assert_eq!(f.st(), State { event: "missing header".into(), ..State::default() });
}

#[test]
fn staged_slot_cut_keeps_the_previous_recovery_and_allows_reinstall() {
    let f = card_with_staged_image("staged-slot-cut");
    let mut card = f.card();
    card.cut = Some(Region::Slot);
    assert_eq!(
        receive(&f, &mut card, &valid_header(&f)).unwrap_err(),
        kexec::StagedError::WriteFailed("power cut".into())
    );
    assert_eq!(&f.magic(), b"KRNL");
    assert_eq!(f.region(Region::Recovery), bootimg(1));
    assert_ne!(f.region(Region::Slot), bootimg(1));
    assert_eq!(
        f.st(),
        State {
            current: Some(entry(1, OLD)),
            kexec: Some(NEW.into()),
            event: SLOT_WRITING.into(),
            slot_incomplete: true,
            ..State::default()
        }
    );
    drop(card);
    assert!(kexec_missed(&f.state(), false).unwrap().unwrap().contains("not installed"));
    assert!(install(&mut f.card(), &f.state(), &package(2, NEW), &[pk()], &RUN).unwrap().written);
    assert_eq!(f.region(Region::Recovery), bootimg(2));
    assert_eq!(f.region(Region::Slot), bootimg(1));
}

#[test]
fn staged_recovery_cut_keeps_stock_selected_and_can_restore_the_previous_image() {
    let f = card_with_staged_image("staged-recovery-cut");
    let mut card = f.card();
    card.cut = Some(Region::Recovery);
    assert_eq!(
        receive(&f, &mut card, &valid_header(&f)).unwrap_err(),
        kexec::StagedError::WriteFailed("power cut".into())
    );
    assert_eq!(&f.magic(), b"KRNL");
    assert!(check_image(&f.region(Region::Recovery)).is_err());
    assert_eq!(f.region(Region::Slot), bootimg(1));
    assert_eq!(
        f.st(),
        State {
            current: Some(entry(2, NEW)),
            previous: Some(entry(1, OLD)),
            trial: Some(0),
            recovery: RecoveryTracking {
                loader_id: Some(image_header(&bootimg(2)).unwrap().id),
                writing: true,
                ..Default::default()
            },
            event: format!("writing build {}", short(NEW)),
            ..State::default()
        }
    );
    assert!(rollback(&mut card, &f.state(), "interrupted install", true, None).unwrap().written);
    assert_eq!(&f.magic(), b"OOSK");
    assert_eq!(f.region(Region::Recovery), bootimg(1));
    assert_eq!(f.st().current, Some(entry(1, OLD)));
    assert_eq!(f.st().trial, None);
}
