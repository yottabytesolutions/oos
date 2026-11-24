// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
mod failures;

const PREVIOUS: &str = "20261105-150000 kernel x";

fn confirmed(name: &str) -> Fake {
    confirmed_in_mode(name, true)
}

fn confirmed_in_mode(name: &str, persist: bool) -> Fake {
    let f = Fake::new(name, Some(1));
    f.poke(SLOT_LBA, &bootimg(3));
    if persist {
        f.card().arm_oos().unwrap();
    }
    State { current: Some(entry(1, OLD)), previous: Some(entry(3, PREVIOUS)), persist, ..State::default() }
        .save(&f.state())
        .unwrap();
    f
}

fn staged_pkg(image: &[u8]) -> Vec<u8> {
    pkg::build_with(
        &SEED,
        NEW,
        "6.12.115+",
        image,
        &[kexec::LINE, kexec::STAGED_LINE, (kexec::STAGED_KEY, &hex(&pk()))],
    )
    .unwrap()
}

fn sender(f: &Fake, card: &mut Card, package: &[u8], opt: &InstallOpt) -> Result<Option<Staged>, String> {
    let (_, image) = pkg::verify(package, &[pk()]).unwrap();
    let (_, _, tree) = kexec::parts(image).unwrap();
    kexec_stage(
        card,
        &f.state(),
        package,
        &[pk()],
        opt,
        BootTrees { booted: Some(tree), running_build: Some(OLD), live: None },
    )
    .map(|(_, start)| start)
}

fn send(f: &Fake) -> Staged {
    sender(f, &mut f.card(), &staged_pkg(&bootimg(2)), &RUN).unwrap().unwrap()
}

fn handoff_file(f: &Fake, start: &Staged, file: &str) -> PathBuf {
    let path = f.dir.join(file.trim_start_matches('/'));
    let bytes = start.initrd.strip_prefix(kexec::parts(&bootimg(2)).unwrap().1).unwrap();
    let item = inflate::cpio_file(bytes, file.trim_start_matches('/')).unwrap();
    std::fs::write(&path, item).unwrap();
    path
}

fn assert_pending(f: &Fake, starting: bool) {
    let state = f.st();
    assert_eq!(state.current, Some(entry(1, OLD)));
    assert_eq!(state.previous, None);
    assert_eq!(state.staged, Some(entry(2, NEW)));
    assert_eq!(state.kexec.as_deref(), starting.then_some(NEW));
    assert_eq!(state.trial, None);
    assert!(state.slot_incomplete);
}

fn assert_discarded(f: &Fake) {
    let state = f.st();
    assert_eq!(state.current, Some(entry(1, OLD)));
    assert_eq!(state.previous, None);
    assert_eq!(state.staged, None);
    assert_eq!(state.kexec, None);
    assert_eq!(state.kexec_payload, None);
    assert_eq!(state.trial, None);
    assert!(state.slot_incomplete);
    assert!(state.rollback_slot_status().unwrap().contains("no rollback image"));
}

#[test]
fn a_trial_blocks_staged_and_legacy_kexec_starts() {
    let f = confirmed("u2-trial");
    let mut state = f.st();
    state.trial = Some(1);
    state.save(&f.state()).unwrap();
    let staged = staged_pkg(&bootimg(2));
    let legacy = kexec_package(2, 7);
    for package in [&staged, &legacy] {
        let error = sender(&f, &mut f.card(), package, &RUN).unwrap_err();
        assert!(error.contains("wait for confirmation"), "{error}");
    }
    assert_eq!(f.st(), state);
    assert_eq!(f.region(Region::Slot), bootimg(3));
    assert_eq!(f.region(Region::Recovery), bootimg(1));
    assert_eq!(&f.magic(), b"OOSK");
}

#[test]
fn a_direct_install_can_retry_a_recovery_readback_failure_without_rollback() {
    for recovered_bytes in [false, true] {
        let f = Fake::new("u4-direct-retry", None);
        let mut card = f.card();
        card.write_fault = Some((Region::Recovery, WriteFault::ReadBackMismatch));
        let package = package(2, NEW);
        let error = install_direct(&mut card, &f.state(), &package, &[pk()], &RUN).unwrap_err();
        assert!(error.contains("READ-BACK MISMATCH"), "{error}");
        assert_eq!(f.st().trial, Some(0));
        assert_eq!(f.st().current, Some(entry(2, NEW)));
        assert_eq!(f.st().previous, None);
        assert_eq!(&f.magic(), b"KRNL");
        assert!(rollback(&mut card, &f.state(), "test", true, None).is_err());
        if recovered_bytes {
            // Also exercise a retry when the recorded write did not finish but recovery now matches.
            f.poke(RECOVERY_LBA, &bootimg(2));
        }
        assert!(install_direct(&mut card, &f.state(), &package, &[pk()], &RUN).unwrap().written);
        assert_eq!(f.region(Region::Recovery), bootimg(2));
        assert_eq!(&f.magic(), b"OOSK");
        assert_eq!(f.st().previous, None);
        assert_eq!(f.st().trial, Some(0));
        assert!(f.st().event.starts_with("installed "));
    }
}

#[test]
fn a_failed_load_clears_the_candidate_and_a_second_stage_can_use_the_slot() {
    let f = confirmed("u4-clear-load");
    send(&f);
    assert_pending(&f, true);
    assert!(install_direct(&mut f.card(), &f.state(), &package(2, NEW), &[pk()], &RUN).is_err());
    kexec_clear(&f.state(), "simulated kexec load failure").unwrap();
    assert_discarded(&f);
    assert!(f.st().event.starts_with("simulated kexec load failure"));
    assert_eq!(f.region(Region::Recovery), bootimg(1));
    assert_eq!(&f.magic(), b"OOSK");
    send(&f);
    assert_pending(&f, true);
    assert_eq!(f.region(Region::Slot), bootimg(2));
}

#[test]
fn staging_requires_the_signed_receiver_key_or_preserves_the_slot() {
    let wrong_key = hex(&ed25519::public_key(&[21; 32]));
    let signer = hex(&pk());
    for key_lines in
        [vec![], vec![(kexec::STAGED_KEY, wrong_key.as_str())], vec![(kexec::STAGED_KEY, signer.as_str()); 2]]
    {
        let f = confirmed("u4-rotation");
        let before = f.st();
        let mut capability = vec![kexec::STAGED_LINE];
        capability.extend(key_lines);
        let no_legacy = pkg::build_with(&SEED, NEW, "test", &bootimg(2), &capability).unwrap();
        assert!(sender(&f, &mut f.card(), &no_legacy, &RUN).unwrap_err().contains("no legacy handoff"));
        assert_eq!(f.st(), before);
        capability.push(kexec::LINE);
        let with_legacy = pkg::build_with(&SEED, NEW, "test", &bootimg(2), &capability).unwrap();
        let start = sender(&f, &mut f.card(), &with_legacy, &RUN).unwrap().unwrap();
        assert_eq!(kexec::receive(&start.cmdline("quiet")), Some(kexec::Receive::Legacy(kexec::Flags::default())));
        assert_eq!(f.st().previous, before.previous);
        assert_eq!(f.st().staged, None);
        assert_eq!(f.region(Region::Slot), bootimg(3));
        assert_eq!(f.region(Region::Recovery), bootimg(1));
        assert_eq!(&f.magic(), b"OOSK");
    }
}

#[test]
fn an_available_live_tree_uses_the_explicit_loader_even_for_an_unchanged_tree() {
    let f = confirmed("u4-same-tree");
    let base = dtb(&[("model", b"DHO\0")], vec![chosen(b"quiet\0")]);
    let section = rsce(&base);
    let mut live_chosen = chosen(b"quiet loader\0");
    live_chosen.set("kaslr-seed", &9u64.to_be_bytes());
    live_chosen.set("linux,initrd-start", &0x1250_0000u64.to_be_bytes());
    live_chosen.set("linux,initrd-end", &0x1350_0000u64.to_be_bytes());
    let live = dtb(&[("model", b"DHO\0")], vec![live_chosen]);
    let package = staged_pkg(&bootimg_second(2, section.clone()));
    let mut card = f.card();
    let trees = |live| BootTrees { booted: Some(&section), running_build: Some(OLD), live: Some(live) };
    let before = f.st();
    let error = kexec_stage(&mut card, &f.state(), &package, &[pk()], &RUN, trees(b"bad FDT")).unwrap_err();
    assert!(error.contains("device tree"), "{error}");
    assert_eq!(f.st(), before);
    assert_eq!(f.region(Region::Slot), bootimg(3));

    let (report, start) = kexec_stage(&mut card, &f.state(), &package, &[pk()], &RUN, trees(&live)).unwrap();
    assert!(report.lines.iter().any(|line| line.contains("kexec-tools")));
    let start = start.unwrap();
    let merged = fdt::Fdt::parse(start.tree.as_deref().unwrap()).unwrap();
    let chosen = &merged.root.kids[0];
    assert_eq!(chosen.prop("bootargs"), Some(&b"quiet loader\0"[..]));
    assert_eq!(chosen.prop("kaslr-seed"), Some(&[0; 8][..]));
    assert_eq!(chosen.prop("linux,initrd-start"), None);
    assert_eq!(chosen.prop("linux,initrd-end"), None);
}

#[test]
fn payload_is_kernel_ramdisk_and_a_small_archive_of_the_three_handoff_files() {
    let f = confirmed("u2-payload");
    let compressed = inflate::tests::gzip_stored(&vec![6; 48_000]);
    let mut ramdisk = compressed;
    ramdisk.resize(ramdisk.len().div_ceil(4) * 4, 0);
    let image = bootimg_parts(2, ramdisk, vec![7; 94_620]);
    let package = staged_pkg(&image);
    let (kernel, ramdisk, tree) = kexec::parts(&image).unwrap();
    let end = pkg::verify_header(&package, &[pk()]).unwrap().data_offset;
    let raw_key = format!("{}\n", hex(&pk()));
    let start = sender(&f, &mut f.card(), &package, &RUN).unwrap().unwrap();
    let archive = start.initrd.strip_prefix(ramdisk).unwrap();
    assert_eq!(start.kernel, kernel);
    assert_eq!(
        archive,
        kexec::newc(&[
            ("oos-kexec.hdr", 0o100600, &package[..end]),
            ("oos-kexec.key", 0o100600, raw_key.as_bytes()),
            ("oos-kexec.dt", 0o100600, tree),
        ])
    );
    assert!((tree.len() + end..=128 * 1024).contains(&archive.len()));
    assert_eq!(start.payload_size(), kernel.len() + ramdisk.len() + archive.len());
    assert!(f.st().event.contains(&format!("{} bytes payload", start.payload_size())));
    assert_eq!(f.region(Region::Slot), image);
    println!("staged fixture: {} bytes payload, {} bytes archive", start.payload_size(), archive.len());
}

#[test]
fn sender_and_receiver_finish_a_trial_with_the_previous_recovery_in_the_slot() {
    let f = confirmed("u2-end-to-end");
    let package = staged_pkg(&bootimg(2));
    let before = f.st();
    let dry_run = InstallOpt { execute: false, ..RUN };
    assert!(sender(&f, &mut f.card(), &package, &dry_run).unwrap().is_none());
    assert_eq!(f.st(), before);
    assert_eq!(f.region(Region::Slot), bootimg(3));
    let start = send(&f);
    let cmdline = start.cmdline("quiet");
    assert_eq!(cmdline, "quiet initramfs_async=0 panic=0 oos.kexec=install,staged");
    assert_eq!(kexec::receive(&cmdline), Some(kexec::Receive::Staged(kexec::Flags::default())));
    assert_pending(&f, true);
    assert!(f.st().rollback_slot_status().unwrap().contains("holds a staged candidate"));
    assert!(rollback(&mut f.card(), &f.state(), "manual", true, None).unwrap_err().contains("staged candidate"));
    assert!(sender(&f, &mut f.card(), &package, &RUN).unwrap_err().contains("already starting by kexec"));
    assert_eq!(f.region(Region::Recovery), bootimg(1));
    assert_eq!(f.region(Region::Slot), bootimg(2));
    assert_eq!(&f.magic(), b"OOSK");

    let header = handoff_file(&f, &start, kexec::HEADER_FILE);
    f.card().disarm_oos().unwrap(); // rcS
    assert!(receive(&f, &mut f.card(), &header).unwrap().written);
    assert_trial(&f);
}

fn assert_trial(f: &Fake) {
    assert_eq!(f.region(Region::Recovery), bootimg(2));
    assert_eq!(f.region(Region::Slot), bootimg(1));
    assert_eq!(&f.magic(), b"OOSK");
    let state = f.st();
    assert_eq!(state.current, Some(entry(2, NEW)));
    assert_eq!(state.previous, Some(entry(1, OLD)));
    assert_eq!(state.kexec, None);
    assert_eq!(state.staged, None);
    assert_eq!(state.trial, Some(0));
    assert!(!state.slot_incomplete);
    assert_eq!(state.rollback_slot_status(), None);
}

#[test]
fn staging_refuses_to_discard_the_only_usable_rollback_image() {
    let f = confirmed("u2-force");
    let before = f.st();
    let force = InstallOpt { force: true, ..RUN };
    let error = sender(&f, &mut f.card(), &staged_pkg(&bootimg(1)), &force).unwrap_err();
    assert!(error.contains("different, valid image in recovery"));
    assert_eq!(f.st(), before);
    assert_eq!(f.region(Region::Recovery), bootimg(1));
    assert_eq!(f.region(Region::Slot), bootimg(3));
    assert_eq!(&f.magic(), b"OOSK");
}
