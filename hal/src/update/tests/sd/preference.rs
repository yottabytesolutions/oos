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
