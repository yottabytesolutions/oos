// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::update::kexec::{simulate_reset, stage_image};
use std::panic::{catch_unwind, AssertUnwindSafe};

const PREVIOUS: &str = "20261105-150000 kernel x";

fn confirmed(name: &str) -> Fake {
    let f = Fake::new(name, Some(1));
    f.poke(SLOT_LBA, &bootimg(3));
    f.card().arm_oos().unwrap();
    State { current: Some(entry(1, OLD)), previous: Some(entry(3, PREVIOUS)), ..State::default() }
        .save(&f.state())
        .unwrap();
    f
}

fn staged_pkg(image: &[u8]) -> Vec<u8> {
    pkg::build_with(&SEED, NEW, "6.12.115+", image, &[kexec::LINE, kexec::STAGED_LINE]).unwrap()
}

fn sender(f: &Fake, card: &mut Card, package: &[u8], opt: &InstallOpt) -> Result<Option<Staged>, String> {
    let (_, image) = pkg::verify(package, &[pk()]).unwrap();
    let (_, _, tree) = kexec::parts(image).unwrap();
    kexec_stage(card, &f.state(), package, &[pk()], opt, BootTrees::new(Some(tree), Some(OLD), None))
        .map(|(_, start)| start)
}

fn send(f: &Fake) -> Staged {
    sender(f, &mut f.card(), &staged_pkg(&bootimg(2)), &RUN).unwrap().unwrap()
}

fn stage_only(f: &Fake) {
    stage_image(&mut f.card(), &f.state(), &mut f.st(), entry(2, NEW), &bootimg(2)).unwrap();
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
fn a_trial_blocks_staged_legacy_and_direct_installs() {
    let f = confirmed("u2-trial");
    let mut state = f.st();
    state.trial = Some(1);
    state.save(&f.state()).unwrap();
    let staged = staged_pkg(&bootimg(2));
    let legacy = kexec_package(2, 7);
    for package in [&staged, &legacy] {
        let error = sender(&f, &mut f.card(), package, &RUN).unwrap_err();
        assert!(error.contains("wait for confirmation"), "{error}");
        let error = install_confirmed(&mut f.card(), &f.state(), package, &[pk()], &RUN).unwrap_err();
        assert!(error.contains("wait for confirmation"), "{error}");
    }
    assert_eq!(f.st(), state);
    assert_eq!(f.region(Region::Slot), bootimg(3));
    assert_eq!(f.region(Region::Recovery), bootimg(1));
    assert_eq!(&f.magic(), b"OOSK");
}

#[test]
fn payload_is_kernel_ramdisk_and_a_small_archive_of_the_three_handoff_files() {
    let f = confirmed("u2-payload");
    let compressed = inflate::tests::gzip_stored(&vec![6; 48_000]);
    let mut ramdisk = compressed;
    ramdisk.resize(ramdisk.len().div_ceil(4) * 4, 0);
    let image = bootimg_parts(2, ramdisk, vec![7; 94_620]);
    let package = pkg::build_with(&SEED, NEW, "6.12.115+", &image, &[kexec::STAGED_LINE]).unwrap();
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
fn a_cut_during_staging_leaves_recovery_selected_and_a_torn_slot_reusable() {
    let f = confirmed("u2-staging-cut");
    let mut card = f.card();
    card.write_fault = Some((Region::Slot, WriteFault::PowerCut));
    let package = staged_pkg(&bootimg(2));
    let panic = catch_unwind(AssertUnwindSafe(|| sender(&f, &mut card, &package, &RUN))).unwrap_err();
    assert_eq!(panic.downcast_ref::<&str>(), Some(&"simulated power cut"));
    assert_pending(&f, false);
    assert!(card.header(Region::Slot).unwrap().is_none());
    let mut torn = bootimg(3);
    let half = torn.len() / 2;
    torn[..half].copy_from_slice(&bootimg(2)[..half]);
    torn[0] = 0;
    assert_eq!(card.read(SLOT_LBA, torn.len()).unwrap(), torn);
    assert_eq!(f.region(Region::Recovery), bootimg(1));
    assert_eq!(&f.magic(), b"OOSK");

    f.card().disarm_oos().unwrap(); // Ordinary rcS after power-on.
    let report = kexec_missed(&f.state(), false).unwrap().unwrap();
    assert!(report.contains("never started"));
    assert_eq!(boot_started(&f.state(), Some(OLD)).unwrap(), Boot::NoTrial);
    assert_discarded(&f);
    assert_eq!(kexec_missed(&f.state(), false).unwrap(), None);
    let start = send(&f);
    assert_eq!(f.region(Region::Slot), bootimg(2));
    let header = handoff_file(&f, &start, kexec::HEADER_FILE);
    assert!(receive(&f, &mut f.card(), &header).unwrap().written);
    assert_trial(&f);
}

#[test]
fn read_back_mismatch_discards_the_candidate_and_prevents_the_jump() {
    let f = confirmed("u2-readback");
    let mut card = f.card();
    card.write_fault = Some((Region::Slot, WriteFault::ReadBackMismatch));
    let error = sender(&f, &mut card, &staged_pkg(&bootimg(2)), &RUN).unwrap_err();
    assert!(error.contains("READ-BACK MISMATCH"), "{error}");
    assert_discarded(&f);
    assert_eq!(f.st().event, error);
    assert!(card.header(Region::Slot).unwrap().is_none());
    let mut corrupted = bootimg(2);
    corrupted[0] = !corrupted[0];
    assert_eq!(card.read(SLOT_LBA, corrupted.len()).unwrap(), corrupted);
    assert_eq!(f.region(Region::Recovery), bootimg(1));
    assert_eq!(&f.magic(), b"OOSK");
    assert_eq!(kexec_missed(&f.state(), false).unwrap(), None);
    assert!(rollback(&mut card, &f.state(), "test", true, None).unwrap_err().contains("no rollback image"));
    send(&f);
    assert_eq!(f.region(Region::Slot), bootimg(2));
}

#[test]
fn watchdog_reset_before_rcs_reports_the_missed_start_once() {
    let f = confirmed("u2-watchdog");
    let start = send(&f);
    simulate_reset(|| {
        assert_pending(&f, true);
        assert_eq!(&f.magic(), b"OOSK");
        assert_eq!(f.region(Region::Recovery), bootimg(1));
        assert_eq!(f.region(Region::Slot), bootimg(2));
    });
    let notice = kexec_missed(&f.state(), false).unwrap().unwrap();
    assert!(notice.contains("did not finish"));
    assert!(notice.contains(&format!("{} bytes kexec payload", start.payload_size())));
    assert_discarded(&f);
    assert_eq!(kexec_missed(&f.state(), false).unwrap(), None);
    assert_eq!(&f.magic(), b"OOSK");
    assert_eq!(f.region(Region::Recovery), bootimg(1));
    assert_eq!(f.region(Region::Slot), bootimg(2));
}

#[test]
fn a_candidate_written_before_the_start_mark_is_reported_once_and_replaced() {
    let f = confirmed("u2-unused");
    stage_only(&f);
    assert_pending(&f, false);
    assert_eq!(f.region(Region::Slot), bootimg(2));
    assert!(kexec_missed(&f.state(), false).unwrap().unwrap().contains("never started"));
    assert_discarded(&f);
    assert_eq!(kexec_missed(&f.state(), false).unwrap(), None);
    assert_eq!(&f.magic(), b"OOSK");
    assert_eq!(f.region(Region::Recovery), bootimg(1));
    let alternate = bootimg(4);
    sender(&f, &mut f.card(), &staged_pkg(&alternate), &RUN).unwrap().unwrap();
    assert_eq!(f.region(Region::Slot), alternate);
}

#[test]
fn an_unhealthy_trial_restores_the_previous_recovery() {
    let f = confirmed("u2-ui-fails");
    let start = send(&f);
    let header = handoff_file(&f, &start, kexec::HEADER_FILE);
    f.card().disarm_oos().unwrap();
    assert!(receive(&f, &mut f.card(), &header).unwrap().written);
    assert_trial(&f);
    assert_eq!(boot_started(&f.state(), Some(NEW)).unwrap(), Boot::Trial(1));
    let Verdict::Bad(reason) = judge(&Health { ui_restarts: UI_RESTARTS_BAD, ..Health::default() }) else {
        panic!("crash loop must fail the trial")
    };
    let expected = f.st().current.unwrap().sha256;
    assert!(rollback(&mut f.card(), &f.state(), &reason, true, Some(expected)).unwrap().written);
    assert_eq!(f.region(Region::Recovery), bootimg(1));
    assert_eq!(f.region(Region::Slot), bootimg(1));
    assert_eq!(&f.magic(), b"OOSK");
    assert_eq!(f.st().current, Some(entry(1, OLD)));
    assert_eq!(f.st().trial, None);
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

fn assert_receive_refuses(f: &Fake, header: &Path, reason: &str) {
    let before = f.st();
    let slot = f.region(Region::Slot);
    let error = refusal(receive(f, &mut f.card(), header));
    assert!(error.contains(reason), "{error}");
    assert_eq!(f.st(), before);
    kexec::record_staged_refusal(&f.state(), &error).unwrap();
    assert_discarded(f);
    let event = f.st().event;
    assert!(event.starts_with(&error));
    assert!(event.ends_with(" bytes kexec payload)"));
    assert_eq!(kexec_missed(&f.state(), false).unwrap(), None);
    assert_eq!(f.region(Region::Slot), slot);
    assert_eq!(f.region(Region::Recovery), bootimg(1));
    assert_eq!(&f.magic(), b"KRNL");
}

#[test]
fn corruption_of_the_header_in_the_jump_refuses_before_installing() {
    let f = confirmed("u2-ram-corruption");
    let header = handoff_file(&f, &send(&f), kexec::HEADER_FILE);
    let mut bytes = std::fs::read(&header).unwrap();
    let at = bytes.windows(4).position(|w| w == b"sig ").unwrap() + 4;
    bytes[at] = if bytes[at] == b'0' { b'1' } else { b'0' };
    std::fs::write(&header, bytes).unwrap();
    f.card().disarm_oos().unwrap();
    assert_receive_refuses(&f, &header, "SIGNATURE INVALID");
}

#[test]
fn damage_to_the_slot_after_staging_refuses_before_installing() {
    let f = confirmed("u2-slot-damage");
    let header = handoff_file(&f, &send(&f), kexec::HEADER_FILE);
    f.poke(SLOT_LBA + 8, b"bad");
    f.card().disarm_oos().unwrap();
    assert_receive_refuses(&f, &header, "SHA-256 mismatch");
}

#[test]
fn a_stale_image_in_the_slot_refuses_before_installing() {
    let f = confirmed("u2-slot-stale");
    let header = handoff_file(&f, &send(&f), kexec::HEADER_FILE);
    f.poke(SLOT_LBA, &bootimg(3));
    f.card().disarm_oos().unwrap();
    assert_receive_refuses(&f, &header, "SHA-256 mismatch");
}

#[test]
fn cut_while_saving_recovery_to_slot_keeps_stock_selected_and_allows_retry() {
    let f = confirmed("u2-receiver-slot-cut");
    let header = handoff_file(&f, &send(&f), kexec::HEADER_FILE);
    let mut card = f.card();
    card.disarm_oos().unwrap();
    card.write_fault = Some((Region::Slot, WriteFault::PowerCut));
    let panic = catch_unwind(AssertUnwindSafe(|| receive(&f, &mut card, &header))).unwrap_err();
    assert_eq!(panic.downcast_ref::<&str>(), Some(&"simulated power cut"));
    assert_eq!(f.region(Region::Recovery), bootimg(1));
    assert_eq!(&f.magic(), b"KRNL");
    assert!(card.header(Region::Slot).unwrap().is_none());
    let state = f.st();
    assert_eq!(state.current, Some(entry(1, OLD)));
    assert_eq!(state.previous, None);
    assert_eq!(state.staged, None);
    assert_eq!(state.kexec.as_deref(), Some(NEW));
    assert_eq!(state.trial, None);
    assert!(state.slot_incomplete);
    assert_eq!(state.event, SLOT_WRITING);

    assert!(kexec_missed(&f.state(), false).unwrap().is_some());
    assert_discarded(&f);
    let header = handoff_file(&f, &send(&f), kexec::HEADER_FILE);
    assert!(receive(&f, &mut card, &header).unwrap().written);
    assert_trial(&f);
}

#[test]
fn cut_while_installing_recovery_keeps_stock_selected_and_the_previous_image() {
    let f = confirmed("u2-receiver-recovery-cut");
    let header = handoff_file(&f, &send(&f), kexec::HEADER_FILE);
    let mut card = f.card();
    card.disarm_oos().unwrap();
    card.cut = Some(Region::Recovery);
    assert_eq!(receive(&f, &mut card, &header).unwrap_err(), kexec::StagedError::WriteFailed("power cut".into()));
    assert_eq!(&f.magic(), b"KRNL");
    assert_eq!(f.region(Region::Slot), bootimg(1));
    assert!(check_image(&f.region(Region::Recovery)).is_err());
    let state = f.st();
    assert_eq!(state.current, Some(entry(2, NEW)));
    assert_eq!(state.previous, Some(entry(1, OLD)));
    assert_eq!(state.staged, None);
    assert_eq!(state.kexec.as_deref(), Some(NEW));
    assert_eq!(state.trial, Some(0));
    assert!(!state.slot_incomplete);

    assert!(rollback(&mut card, &f.state(), "interrupted install", true, None).unwrap().written);
    assert_eq!(&f.magic(), b"OOSK");
    assert_eq!(f.region(Region::Recovery), bootimg(1));
    assert_eq!(f.region(Region::Slot), bootimg(1));
    assert_eq!(f.st().current, Some(entry(1, OLD)));
    assert_eq!(f.st().trial, None);
}
