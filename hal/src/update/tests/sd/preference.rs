// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

#[test]
fn an_explicit_preference_shares_the_cli_plan_and_preserves_other_config() {
    let f = active("sd-explicit");
    let config = f.dir.join("oos/system.conf");
    let original = "# Device\nntp = on\nsd.mode = broken\nplugin = value\nsd.mode = hs\n";
    std::fs::write(&config, original).unwrap();
    let mut card = f.card();
    let before = f.st();
    let hs = f.region(Region::Recovery);
    let mode = Some(Mode::Sdr104);

    assert!(!sd::restage_preference(&mut card, &f.state(), mode, false).unwrap().written);
    assert_eq!(std::fs::read_to_string(&config).unwrap(), original);
    assert_eq!(f.st(), before);
    assert_eq!(f.region(Region::Recovery), hs);

    assert!(sd::restage_preference(&mut card, &f.state(), mode, true).unwrap().written);
    assert_eq!(std::fs::read_to_string(&config).unwrap(), "# Device\nntp = on\nplugin = value\nsd.mode = sdr104\n");
    assert_eq!(f.region(Region::Recovery), sd::image(&hs, Mode::Sdr104).unwrap());
    assert_eq!(f.st().trial, Some(0));
    assert_eq!(f.region(Region::Slot), hs);
    assert_eq!(&f.magic(), b"OOSK");
}

#[test]
fn trial_pending_handoff_staged_candidate_and_busy_lock_refuse_before_writing() {
    let f = active("sd-refused");
    set_mode(&f, "hs");
    let confirmed = f.st();
    let before = card_bytes(&f);
    for (trial, kexec, staged, reason) in [
        (Some(0), None, None, "on trial"),
        (None, Some(KX_BUILD.into()), None, "starting by kexec"),
        (None, None, confirmed.current.clone(), "staged for kexec"),
        (None, None, None, "busy"),
    ] {
        let state = State { trial, kexec, staged, ..confirmed.clone() };
        state.save(&f.state()).unwrap();
        let expected = f.st();
        let _busy = (reason == "busy").then(|| lock(&f.state()).unwrap());
        let error = sd::restage_preference(&mut f.card(), &f.state(), Some(Mode::Sdr104), true).unwrap_err();
        assert!(error.contains(reason), "{error}");
        assert_eq!(sd::requested(&f.state()).unwrap(), Mode::Hs);
        assert_eq!(f.st(), expected);
        assert_eq!(card_bytes(&f), before);
    }
}

#[test]
fn a_cut_before_the_trial_leaves_recovery_usable_and_the_restage_repeatable() {
    for (name, fault, slot_verified) in [
        ("before-slot-write", (SLOT_LBA, WriteFault::CutBeforeWrite), false),
        ("during-slot-write", (SLOT_LBA, WriteFault::PowerCut), false),
        ("after-slot-readback", (SLOT_LBA, WriteFault::CutAfterReadBack), false),
        ("slot-recorded-before-disarm", (BOOT_LBA, WriteFault::CutBeforeWrite), true),
    ] {
        let f = active(name);
        f.card().arm_oos().unwrap();
        let hs = f.region(Region::Recovery);
        let mut latched = f.st();
        latched.sd.failed = true;
        latched.sd.missed = 2;
        latched.save(&f.state()).unwrap();
        let mut card = f.card();
        cut_restage(&f, &mut card, fault);

        let state = f.st();
        assert_eq!(sd::requested(&f.state()).unwrap(), Mode::Sdr104, "{name}");
        assert!(!state.sd.failed, "{name}");
        assert_eq!(state.sd.missed, 0, "{name}");
        assert_eq!(&f.magic(), b"OOSK", "{name}");
        assert_eq!(f.region(Region::Recovery), hs, "{name}");
        assert_eq!(state.current, latched.current, "{name}");
        assert_eq!(state.trial, None, "{name}");
        assert_eq!(state.previous.is_some(), slot_verified, "{name}");
        assert_eq!(state.slot_incomplete, !slot_verified, "{name}");
        assert!(sd::restage(&mut card, &f.state(), true).unwrap().written, "{name}");
    }
}

#[test]
fn a_cut_with_a_trial_write_open_keeps_stock_selected_and_the_hs_slot_verified() {
    for (name, fault) in [
        ("before-recovery-write", WriteFault::CutBeforeWrite),
        ("during-recovery-write", WriteFault::PowerCut),
        ("after-recovery-readback", WriteFault::CutAfterReadBack),
    ] {
        let f = active(name);
        let hs = f.region(Region::Recovery);
        let mut card = f.card();
        cut_restage(&f, &mut card, (RECOVERY_LBA, fault));

        assert_eq!(sd::requested(&f.state()).unwrap(), Mode::Sdr104, "{name}");
        assert_eq!(&f.magic(), b"KRNL", "{name}");
        assert_eq!(f.region(Region::Slot), hs, "{name}");
        assert!(f.st().interrupted_recovery_write(), "{name}");
        rollback(&mut card, &f.state(), "restage interrupted", true, None).unwrap();
        assert_eq!(f.region(Region::Recovery), hs, "{name}");
        assert_eq!(f.st().trial, None, "{name}");
        assert_eq!(f.st().sd.missed, 0, "{name}");
        assert!(sd::restage(&mut card, &f.state(), true).unwrap().written, "{name}");
    }
}

#[test]
fn a_cut_after_the_trial_is_recorded_leaves_a_verified_image_to_arm() {
    let f = active("trial-recorded-before-arm");
    let hs = f.region(Region::Recovery);
    let mut card = f.card();
    card.write_fault_at = Some((BOOT_LBA * SECTOR, WriteFault::CutBeforeWrite));
    let report = sd::restage_preference(&mut card, &f.state(), Some(Mode::Sdr104), true).unwrap();
    assert!(report.written);
    assert!(report.lines.last().unwrap().contains("WARNING"));
    assert_eq!(sd::requested(&f.state()).unwrap(), Mode::Sdr104);
    assert_eq!(&f.magic(), b"KRNL");
    assert_eq!(f.region(Region::Recovery), sd::image(&hs, Mode::Sdr104).unwrap());
    assert_eq!(f.region(Region::Slot), hs);
    assert_eq!(f.st().trial, Some(0));
    assert!(!f.st().interrupted_recovery_write());
    assert!(arm_by_hand(&mut card, &f.state()).unwrap());
    let fast = super::trial_evidence::observation(&f, "sdr104", "sdr104", false);
    assert_eq!(boot_started_with_card(&f.state(), Some(KX_BUILD), fast).unwrap(), Boot::Trial(1));
    assert!(confirm_running(&f.state(), None, Some(KX_BUILD)).unwrap());
}

fn cut_restage(f: &Fake, card: &mut Card, (lba, fault): (u64, WriteFault)) {
    use std::panic::{catch_unwind, AssertUnwindSafe};
    let torn = matches!(fault, WriteFault::PowerCut);
    card.write_fault_at = Some((lba * SECTOR, fault));
    let result = catch_unwind(AssertUnwindSafe(|| sd::restage_preference(card, &f.state(), Some(Mode::Sdr104), true)));
    if torn {
        assert_eq!(result.unwrap_err().downcast_ref::<&str>(), Some(&"simulated power cut"));
    } else {
        assert!(result.unwrap().unwrap_err().contains("power cut"));
    }
}
