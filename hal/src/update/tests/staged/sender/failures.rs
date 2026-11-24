// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::update::kexec::{simulate_reset, stage_image};
use std::panic::{catch_unwind, AssertUnwindSafe};

fn stage_only(f: &Fake) {
    stage_image(&mut f.card(), &f.state(), &mut f.st(), entry(2, NEW), &bootimg(2)).unwrap();
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
fn a_cut_during_staging_leaves_recovery_selected_and_a_torn_slot_reusable() {
    for_boot_modes("u2-staging-cut", |f, selected| {
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
        assert_eq!(&f.magic(), selected);

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
    });
}

#[test]
fn read_back_mismatch_discards_the_candidate_and_prevents_the_jump() {
    for_boot_modes("u2-readback", |f, selected| {
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
        assert_eq!(&f.magic(), selected);
        assert_eq!(kexec_missed(&f.state(), false).unwrap(), None);
        assert!(rollback(&mut card, &f.state(), "test", true, None).unwrap_err().contains("no rollback image"));
        send(&f);
        assert_eq!(f.region(Region::Slot), bootimg(2));
    });
}

#[test]
fn watchdog_reset_before_rcs_reports_the_missed_start_once() {
    for_boot_modes("u2-watchdog", |f, selected| {
        let start = send(&f);
        simulate_reset(|| {
            assert_pending(&f, true);
            assert_eq!(&f.magic(), selected);
            assert_eq!(f.region(Region::Recovery), bootimg(1));
            assert_eq!(f.region(Region::Slot), bootimg(2));
        });
        let notice = kexec_missed(&f.state(), false).unwrap().unwrap();
        assert!(notice.contains("did not finish"));
        assert!(notice.contains(&format!("{} bytes kexec payload", start.payload_size())));
        assert_discarded(&f);
        assert_eq!(kexec_missed(&f.state(), false).unwrap(), None);
        assert_eq!(&f.magic(), selected);
        assert_eq!(f.region(Region::Recovery), bootimg(1));
        assert_eq!(f.region(Region::Slot), bootimg(2));
    });
}

#[test]
fn a_candidate_written_before_the_start_mark_is_reported_once_and_replaced() {
    for_boot_modes("u2-unused", |f, selected| {
        stage_only(&f);
        assert_pending(&f, false);
        assert_eq!(f.region(Region::Slot), bootimg(2));
        assert!(kexec_missed(&f.state(), false).unwrap().unwrap().contains("never started"));
        assert_discarded(&f);
        assert_eq!(kexec_missed(&f.state(), false).unwrap(), None);
        assert_eq!(&f.magic(), selected);
        assert_eq!(f.region(Region::Recovery), bootimg(1));
        let alternate = bootimg(4);
        sender(&f, &mut f.card(), &staged_pkg(&alternate), &RUN).unwrap().unwrap();
        assert_eq!(f.region(Region::Slot), alternate);
    });
}

#[test]
fn an_unhealthy_trial_restores_the_previous_recovery() {
    for_boot_modes("u2-ui-fails", |f, _| {
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
    });
}

#[test]
fn corruption_of_the_header_in_the_jump_refuses_before_installing() {
    for_boot_modes("u2-ram-corruption", |f, _| {
        let header = handoff_file(&f, &send(&f), kexec::HEADER_FILE);
        let mut bytes = std::fs::read(&header).unwrap();
        let at = bytes.windows(4).position(|w| w == b"sig ").unwrap() + 4;
        bytes[at] = if bytes[at] == b'0' { b'1' } else { b'0' };
        std::fs::write(&header, bytes).unwrap();
        f.card().disarm_oos().unwrap();
        assert_receive_refuses(&f, &header, "SIGNATURE INVALID");
    });
}

#[test]
fn damage_to_the_slot_after_staging_refuses_before_installing() {
    for_boot_modes("u2-slot-damage", |f, _| {
        let header = handoff_file(&f, &send(&f), kexec::HEADER_FILE);
        f.poke(SLOT_LBA + 8, b"bad");
        f.card().disarm_oos().unwrap();
        assert_receive_refuses(&f, &header, "SHA-256 mismatch");
    });
}

#[test]
fn a_stale_image_in_the_slot_refuses_before_installing() {
    for_boot_modes("u2-slot-stale", |f, _| {
        let header = handoff_file(&f, &send(&f), kexec::HEADER_FILE);
        f.poke(SLOT_LBA, &bootimg(3));
        f.card().disarm_oos().unwrap();
        assert_receive_refuses(&f, &header, "SHA-256 mismatch");
    });
}

#[test]
fn cut_while_saving_recovery_to_slot_keeps_stock_selected_and_allows_retry() {
    for_boot_modes("u2-receiver-slot-cut", |f, _| {
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
    });
}

#[test]
fn cut_while_installing_recovery_keeps_stock_selected_and_the_previous_image() {
    for_boot_modes("u2-receiver-recovery-cut", |f, _| {
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
        assert_eq!(state.kexec, None);
        assert_eq!(state.trial, Some(0));
        assert!(!state.slot_incomplete);

        assert!(rollback(&mut card, &f.state(), "interrupted install", true, None).unwrap().written);
        assert_eq!(&f.magic(), b"OOSK");
        assert_eq!(f.region(Region::Recovery), bootimg(1));
        assert_eq!(f.region(Region::Slot), bootimg(1));
        assert_eq!(f.st().current, Some(entry(1, OLD)));
        assert_eq!(f.st().trial, None);
    });
}

#[test]
fn a_failed_receive_allows_direct_retry_without_replacing_the_rollback_image() {
    let (f, error) = failed_receiver("receiver-direct-retry");
    // The CLI records its notice even when the attempted automatic restore fails.
    kexec::record_staged_refusal(&f.state(), &error).unwrap();
    assert_eq!(f.st().event, error);
    assert_eq!(boot_started(&f.state(), Some(OLD)).unwrap(), Boot::NoTrial);

    let mut card = f.card();
    let before = f.st();
    let older = package(3, "20261105-165000");
    let error = install_direct(&mut card, &f.state(), &older, &[pk()], &RUN).unwrap_err();
    assert!(error.contains("older than the installed"), "{error}");
    assert_eq!(f.st(), before);
    assert_eq!(&f.magic(), b"KRNL");

    let newer = package(4, "20261105-180000");
    assert!(install_direct(&mut card, &f.state(), &newer, &[pk()], &RUN).unwrap().written);
    assert_eq!(f.st().previous, Some(entry(1, OLD)));
    assert_eq!(f.region(Region::Slot), bootimg(1));
    assert_eq!(f.region(Region::Recovery), bootimg(4));
    assert_eq!(&f.magic(), b"OOSK");
}

#[test]
fn a_failed_receive_cannot_retry_after_the_trial_has_started() {
    let (f, _) = failed_receiver("receiver-tried");
    let mut card = f.card();
    let candidate = package(2, NEW);
    for (count, boot) in [(1, Boot::Trial(1)), (2, Boot::Rollback)] {
        assert_eq!(boot_started(&f.state(), Some(NEW)).unwrap(), boot);
        let before = f.st();
        assert_eq!(before.trial, Some(count));
        let error = install_direct(&mut card, &f.state(), &candidate, &[pk()], &RUN).unwrap_err();
        assert!(error.contains("wait for confirmation"), "{error}");
        assert_eq!(f.st(), before);
        assert_eq!(f.region(Region::Recovery), bootimg(2));
        assert_eq!(f.region(Region::Slot), bootimg(1));
        assert_eq!(&f.magic(), b"KRNL");
    }
}

fn failed_receiver(name: &str) -> (Fake, String) {
    let f = confirmed(name);
    let header = handoff_file(&f, &send(&f), kexec::HEADER_FILE);
    let mut card = f.card();
    card.disarm_oos().unwrap();
    card.write_fault = Some((Region::Recovery, WriteFault::CutAfterReadBack));
    let kexec::StagedError::WriteFailed(error) = receive(&f, &mut card, &header).unwrap_err() else {
        panic!("expected a failure after the recovery write");
    };
    assert_eq!(f.region(Region::Recovery), bootimg(2));
    assert_eq!(f.region(Region::Slot), bootimg(1));
    (f, error)
}

fn for_boot_modes(name: &str, test: impl Fn(Fake, &[u8; 4])) {
    for persist in [false, true] {
        let selected = if persist { b"OOSK" } else { b"KRNL" };
        test(confirmed_in_mode(name, persist), selected);
    }
}

#[test]
fn staged_sender_keeps_the_slot_when_recovery_is_damaged_or_non_oos() {
    let mut damaged = bootimg(1);
    damaged[3000] ^= 1;
    assert!(check_image(&damaged).unwrap_err().contains("id mismatch"));
    let mut non_oos = vec![0; damaged.len()];
    non_oos[..4].copy_from_slice(b"KRNL");
    for recovery in [damaged, non_oos] {
        for_boot_modes("u4-bad-recovery", |f, selected| {
            f.poke(RECOVERY_LBA, &recovery);
            let before = f.st();
            let mut card = f.card();
            let error = sender(&f, &mut card, &staged_pkg(&bootimg(2)), &RUN).unwrap_err();
            assert!(error.contains("different, valid image in recovery"), "{error}");
            assert_eq!(f.st(), before);
            assert_eq!(card.read(RECOVERY_LBA, recovery.len()).unwrap(), recovery);
            assert_eq!(f.region(Region::Slot), bootimg(3));
            assert_eq!(&f.magic(), selected);
        });
    }
}
