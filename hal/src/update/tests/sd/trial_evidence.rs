// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

const NEXT: &str = "20261109-090000";

pub(super) fn observation(f: &Fake, mode: &str, ceiling: &str, kexec: bool) -> crate::bootcard::Status {
    let path = f.dir.join("negotiated");
    std::fs::write(
        &path,
        format!(
            "mode={mode}\nexpected=hs\nstate=unverified\nreason=none\nceiling={ceiling}\ndegraded_in_a_row=0\nkexec={}\n",
            u8::from(kexec)
        ),
    )
    .unwrap();
    crate::bootcard::read_state(&path)
}

#[test]
fn a_cold_sdr104_tree_with_known_slower_timing_latches_fallback() {
    let f = active("sd-ordinary-evidence");
    set_mode(&f, "sdr104");
    sd::restage(&mut f.card(), &f.state(), true).unwrap();
    let trial = f.st();
    for (mark, running, mode, ceiling, kexec, failed) in [
        (None, Some(KX_BUILD), "hs", "sdr104", false, true),
        (Some(Mode::Hs), Some(KX_BUILD), "hs", "hs", false, false),
        (Some(Mode::Sdr104), None, "hs", "sdr104", false, false),
        (Some(Mode::Sdr104), Some("different"), "hs", "hs", false, false),
        (Some(Mode::Sdr104), Some(KX_BUILD), "hs", "sdr104", true, false),
        (Some(Mode::Sdr104), Some(KX_BUILD), "hs", "hs", false, false),
        (Some(Mode::Sdr104), Some(KX_BUILD), "hs", "unknown", false, false),
        (Some(Mode::Sdr104), Some(KX_BUILD), "unknown", "sdr104", false, false),
        (Some(Mode::Sdr104), Some(KX_BUILD), "sdr104", "sdr104", false, false),
        (Some(Mode::Sdr104), Some(KX_BUILD), "hs", "sdr104", false, true),
        (Some(Mode::Sdr104), Some(KX_BUILD), "sdr12", "sdr104", false, true),
        (Some(Mode::Sdr104), Some(KX_BUILD), "sdr25", "sdr104", false, true),
        (Some(Mode::Sdr104), Some(KX_BUILD), "sdr50", "sdr104", false, true),
        (Some(Mode::Sdr104), Some(KX_BUILD), "ddr50", "sdr104", false, true),
        (Some(Mode::Sdr104), Some(KX_BUILD), "legacy", "sdr104", false, true),
    ] {
        let observed = observation(&f, mode, ceiling, kexec);
        let mut state = trial.clone();
        state.sd.trial = mark;
        state.save(&f.state()).unwrap();
        boot_started_with_card(&f.state(), running, observed).unwrap();
        assert_eq!(f.st().sd.failed, failed, "{mark:?} {running:?} {observed:?}");
    }
}

#[test]
fn confirmation_preserves_the_recorded_fallback_for_the_next_install() {
    let f = active("sd-confirm-fallback");
    let (_, candidate, package) = candidate(&f);
    sd::restage(&mut f.card(), &f.state(), true).unwrap();
    let observed = observation(&f, "hs", "sdr104", false);
    assert_eq!(boot_started_with_card(&f.state(), Some(KX_BUILD), observed).unwrap(), Boot::Trial(1));
    let sha = f.st().current.unwrap().sha256;
    assert!(confirm_with_sd_mode(&f.state(), sha, observed.mode()).unwrap());
    install_direct(&mut f.card(), &f.state(), &package, &[pk()], &RUN).unwrap();
    assert!(f.st().sd.failed);
    assert_eq!(f.region(Region::Recovery), candidate);
}

#[test]
fn kexec_timing_is_associated_only_with_the_matching_handoff() {
    let f = active("sd-kexec-evidence");
    let (_, _, package) = candidate(&f);
    stage(&f, &mut f.card(), &package);
    let pending = f.st();
    for (build, mark, mode, ceiling, kexec, count) in [
        (None, Some(Mode::Sdr104), "hs", "sdr104", true, 0),
        (Some(NEXT), None, "hs", "sdr104", true, 0),
        (Some(NEXT), Some(Mode::Hs), "hs", "sdr104", true, 0),
        (Some("old"), Some(Mode::Sdr104), "hs", "sdr104", true, 1),
        (Some(NEXT), Some(Mode::Sdr104), "hs", "sdr104", false, 1),
        (Some(NEXT), Some(Mode::Sdr104), "hs", "hs", true, 1),
        (Some(NEXT), Some(Mode::Sdr104), "unknown", "sdr104", true, 1),
        (Some(NEXT), Some(Mode::Sdr104), "sdr104", "sdr104", true, 0),
        (Some(NEXT), Some(Mode::Sdr104), "hs", "sdr104", true, 0),
    ] {
        let observed = observation(&f, mode, ceiling, kexec);
        let mut state = pending.clone();
        state.kexec = build.map(str::to_owned);
        state.sd.trial = mark;
        state.save(&f.state()).unwrap();
        sd::observe_handoff(&f.state(), NEXT, observed).unwrap();
        kexec_missed(&f.state(), false).unwrap();
        assert!(!f.st().sd.failed, "{build:?} {mark:?} {observed:?}");
        assert_eq!(f.st().sd.missed, count, "{build:?} {mark:?} {observed:?}");
    }
}

fn candidate(f: &Fake) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    set_mode(f, "sdr104");
    let previous = f.region(Region::Recovery);
    let image = bootimg_second(2, rsce(&base_tree()));
    let package = pkg::build_with(
        &SEED,
        NEXT,
        "test",
        &image,
        &[kexec::LINE, kexec::STAGED_LINE, (kexec::STAGED_KEY, &hex(&pk()))],
    )
    .unwrap();
    (previous, image, package)
}

fn stage(f: &Fake, card: &mut Card, package: &[u8]) -> Staged {
    let previous = f.region(Region::Recovery);
    let (_, _, tree) = kexec::parts(&previous).unwrap();
    let trees = BootTrees { booted: Some(tree), running_build: Some(KX_BUILD), live: Some(&base_tree()) };
    kexec_stage(card, &f.state(), package, &[pk()], &RUN, trees).unwrap().1.unwrap()
}

#[test]
fn a_marked_warm_slow_start_leaves_the_cold_start_to_answer() {
    let f = active("sd-warm");
    let (_, _, package) = candidate(&f);
    stage(&f, &mut f.card(), &package);
    kexec_missed(&f.state(), false).unwrap();
    sd::restage(&mut f.card(), &f.state(), true).unwrap();
    let mark = restart_mark::restart_mark(&f.state());
    std::fs::write(&mark, format!("{KX_BUILD}\n")).unwrap();
    let slow = observation(&f, "hs", "sdr104", false);
    assert_eq!(boot_started_with_card(&f.state(), Some(KX_BUILD), slow).unwrap(), Boot::Trial(1));
    assert!(!mark.exists());
    let sha = f.st().current.unwrap().sha256;
    assert!(confirm_with_sd_mode(&f.state(), sha, slow.mode()).unwrap());
    assert!(!f.st().sd.failed);
    assert_eq!(f.st().sd.missed, 1);

    std::fs::write(&mark, NEXT).unwrap();
    assert_eq!(boot_started_with_card(&f.state(), Some(KX_BUILD), slow).unwrap(), Boot::NoTrial);
    assert!(f.st().sd.failed);
}

#[test]
fn a_missed_handoff_and_an_unobserved_rollback_latch_only_once_each() {
    let f = active("sd-missed-rollback");
    let (hs, _, package) = candidate(&f);
    stage(&f, &mut f.card(), &package);
    kexec_missed(&f.state(), false).unwrap();
    assert!(!f.st().sd.failed);
    sd::restage(&mut f.card(), &f.state(), true).unwrap();
    assert_eq!(boot_started(&f.state(), Some(KX_BUILD)).unwrap(), Boot::Trial(1));
    assert_eq!(boot_started(&f.state(), Some(KX_BUILD)).unwrap(), Boot::Rollback);
    let mut card = f.card();
    card.cut = Some(Region::Recovery);
    assert!(rollback(&mut card, &f.state(), "trial did not confirm", true, None).is_err());
    assert_eq!(f.st().sd.missed, 2);
    assert!(rollback(&mut card, &f.state(), "retry rollback", true, None).unwrap().written);
    assert!(f.st().sd.failed);
    assert_eq!(f.st().sd.missed, 2);
    assert_eq!(f.region(Region::Recovery), hs);
}

#[test]
fn a_fast_start_resets_misses_even_during_kexec() {
    for kexec in [false, true] {
        let f = active(if kexec { "sd-fast-kexec" } else { "sd-fast-cold" });
        let (_, _, package) = candidate(&f);
        let mut card = f.card();
        stage(&f, &mut card, &package);
        kexec_missed(&f.state(), false).unwrap();
        let fast = observation(&f, "sdr104", "sdr104", kexec);
        if kexec {
            stage(&f, &mut card, &package);
            sd::observe_handoff(&f.state(), NEXT, fast).unwrap();
            kexec_missed(&f.state(), false).unwrap();
        } else {
            boot_started_with_card(&f.state(), Some(KX_BUILD), fast).unwrap();
        }
        assert_eq!(f.st().sd.missed, 0);
        stage(&f, &mut card, &package);
        kexec_missed(&f.state(), false).unwrap();
        assert!(!f.st().sd.failed);
        assert_eq!(f.st().sd.missed, 1);
    }
}

#[test]
fn fast_timing_from_a_replacement_boot_resets_the_miss_from_the_dropped_trial() {
    let f = active("sd-fast-after-dropped-trial");
    let (_, image, _) = candidate(&f);
    sd::restage(&mut f.card(), &f.state(), true).unwrap();
    f.poke(RECOVERY_LBA, &sd::image(&image, Mode::Sdr104).unwrap());
    let fast = observation(&f, "sdr104", "sdr104", false);
    assert!(matches!(boot_started_with_card(&f.state(), Some(NEXT), fast).unwrap(), Boot::Dropped(_)));
    assert_eq!(f.st().sd.missed, 0);
    assert!(!f.st().sd.failed);
}

#[test]
fn only_an_explicit_sdr104_retry_clears_a_latch() {
    let f = active("sd-sticky-latch");
    let (_, _, package) = candidate(&f);
    for _ in 0..2 {
        stage(&f, &mut f.card(), &package);
        kexec_missed(&f.state(), false).unwrap();
    }
    let mut card = f.card();
    sd::restage_preference(&mut card, &f.state(), Some(Mode::Hs), true).unwrap();
    assert_eq!(f.st().sd.missed, 2);
    assert!(f.st().sd.failed);
    let fast = observation(&f, "sdr104", "sdr104", false);
    boot_started_with_card(&f.state(), Some(KX_BUILD), fast).unwrap();
    assert_eq!(f.st().sd.missed, 0);
    assert!(f.st().sd.failed);
    let retry = |card: &mut Card, execute| sd::restage_preference(card, &f.state(), Some(Mode::Sdr104), execute);
    assert!(!retry(&mut card, false).unwrap().written);
    assert!(f.st().sd.failed);
    assert!(retry(&mut card, true).unwrap().written);
    assert!(!f.st().sd.failed);
}

#[test]
fn cuts_through_the_sender_handoff_keep_sdr104_for_the_next_install() {
    use std::panic::{catch_unwind, AssertUnwindSafe};
    for cut in ["before-slot", "during-slot", "after-slot", "before-jump", "before-receiver", "after-observation"] {
        let f = active(cut);
        let (previous, image, package) = candidate(&f);
        let mut card = f.card();
        match cut {
            "before-slot" => {}
            "during-slot" => {
                card.write_fault = Some((Region::Slot, WriteFault::PowerCut));
                let panic = catch_unwind(AssertUnwindSafe(|| stage(&f, &mut card, &package))).unwrap_err();
                assert_eq!(panic.downcast_ref::<&str>(), Some(&"simulated power cut"));
            }
            "after-slot" => {
                let entry = Entry { build: NEXT.into(), sha256: hash::sha256(&image), len: image.len() as u64 };
                kexec::stage_image(&mut card, &f.state(), &mut f.st(), entry, &image).unwrap();
            }
            "before-jump" => {
                stage(&f, &mut card, &package);
            }
            "before-receiver" => {
                stage(&f, &mut card, &package);
                kexec::simulate_reset(|| assert_eq!(f.st().kexec.as_deref(), Some(NEXT)));
            }
            "after-observation" => {
                stage(&f, &mut card, &package);
                sd::observe_handoff(&f.state(), NEXT, observation(&f, "sdr104", "sdr104", true)).unwrap();
            }
            _ => unreachable!(),
        }
        assert_eq!(f.region(Region::Recovery), previous, "{cut}");
        kexec_missed(&f.state(), false).unwrap();
        assert!(!f.st().sd.failed, "{cut}");
        install_direct(&mut card, &f.state(), &package, &[pk()], &RUN).unwrap();
        assert_eq!(f.region(Region::Recovery), sd::image(&image, Mode::Sdr104).unwrap(), "{cut}");
    }
}

#[test]
fn cuts_during_and_after_receiver_writes_keep_sdr104_for_the_next_install() {
    use std::panic::{catch_unwind, AssertUnwindSafe};
    for cut in ["receiver-slot", "receiver-recovery", "before-ordinary-start"] {
        let f = active(cut);
        let (_, image, package) = candidate(&f);
        let mut card = f.card();
        let staged = stage(&f, &mut card, &package);
        let archive = staged.initrd.strip_prefix(kexec::parts(&image).unwrap().1).unwrap();
        let header = f.dir.join("header");
        std::fs::write(&header, inflate::cpio_file(archive, "oos-kexec.hdr").unwrap()).unwrap();
        sd::observe_handoff(&f.state(), NEXT, observation(&f, "sdr104", "sdr104", true)).unwrap();
        match cut {
            "receiver-slot" => {
                card.write_fault = Some((Region::Slot, WriteFault::PowerCut));
                let panic = catch_unwind(AssertUnwindSafe(|| receive(&f, &mut card, &header))).unwrap_err();
                assert_eq!(panic.downcast_ref::<&str>(), Some(&"simulated power cut"));
                kexec_missed(&f.state(), false).unwrap();
            }
            "receiver-recovery" => {
                card.cut = Some(Region::Recovery);
                assert!(matches!(receive(&f, &mut card, &header), Err(kexec::StagedError::WriteFailed(_))));
            }
            "before-ordinary-start" => {
                assert!(receive(&f, &mut card, &header).unwrap().written);
                assert_eq!(f.st().trial, Some(0));
                assert!(
                    rollback(&mut card, &f.state(), "operator declines pending trial", true, None).unwrap().written
                );
            }
            _ => unreachable!(),
        }
        assert!(!f.st().sd.failed, "{cut}");
        install_direct(&mut card, &f.state(), &package, &[pk()], &RUN).unwrap();
        assert_eq!(f.region(Region::Recovery), sd::image(&image, Mode::Sdr104).unwrap(), "{cut}");
    }
}

fn receive(f: &Fake, card: &mut Card, header: &Path) -> Result<Report, kexec::StagedError> {
    kexec::install_staged(card, &f.state(), header, NEXT, &[pk()], &RUN)
}
