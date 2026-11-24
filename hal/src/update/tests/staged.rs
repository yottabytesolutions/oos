// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

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

fn receive(f: &Fake, card: &mut Card, header_file: &Path, keys: &[[u8; 32]]) -> Result<Report, String> {
    let package = kexec::staged_package(card, header_file, NEW, keys)?;
    install(card, &f.state(), &package, keys, &RUN)
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

    assert!(receive(&staged, &mut staged.card(), &valid_header(&staged), &[pk()]).unwrap().written);
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

    let cases = [
        ("hash", Some(&valid), Some(vec![0; bootimg(2).len()]), false, "SHA-256 mismatch"),
        ("wrong-build", Some(&wrong_build), None, false, "does not match running build"),
        ("no-header", None, None, false, "reading the staged header"),
        ("short-slot", Some(&valid), None, true, "reading the staged image from the rollback slot"),
        ("stale-slot", Some(&valid), Some(bootimg(3)), false, "SHA-256 mismatch"),
        ("signature", Some(&bad_signature), None, false, "SIGNATURE INVALID"),
        ("unannounced", Some(&unannounced), None, false, "no 'trial staged' line"),
        ("trailing", Some(&trailing), None, false, "trailing bytes"),
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
        let refusal = receive(&f, &mut card, &path, &[pk()]).unwrap_err();
        assert!(refusal.contains(reason), "{name}: {refusal}");
        assert_eq!(&f.magic(), b"KRNL", "{name}");
        assert_eq!(f.region(Region::Recovery), bootimg(1), "{name}");
        assert_eq!(card.read(SLOT_LBA, slot_len).unwrap(), original_slot, "{name}");
        assert_eq!(f.st(), before, "{name}");
        kexec::record_staged_refusal(&f.state(), &refusal).unwrap();
        assert_eq!(f.st(), State { kexec: None, event: refusal, ..before }, "{name}");
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
    assert_eq!(receive(&f, &mut card, &valid_header(&f), &[pk()]).unwrap_err(), "power cut");
    assert_eq!(&f.magic(), b"KRNL");
    assert_eq!(f.region(Region::Recovery), bootimg(1));
    assert_ne!(f.region(Region::Slot), bootimg(1));
    assert_eq!(
        f.st(),
        State { current: Some(entry(1, OLD)), kexec: Some(NEW.into()), event: SLOT_WRITING.into(), ..State::default() }
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
    assert_eq!(receive(&f, &mut card, &valid_header(&f), &[pk()]).unwrap_err(), "power cut");
    assert_eq!(&f.magic(), b"KRNL");
    assert!(check_image(&f.region(Region::Recovery)).is_err());
    assert_eq!(f.region(Region::Slot), bootimg(1));
    assert_eq!(
        f.st(),
        State {
            current: Some(entry(2, NEW)),
            previous: Some(entry(1, OLD)),
            trial: Some(0),
            kexec: Some(NEW.into()),
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
