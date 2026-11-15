// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::update::unit as identity;

const UNIT_RUN: InstallOpt<'static> = InstallOpt { running_unit: "fake-unit", ..RUN };

fn unit_package(unit: &str) -> Vec<u8> {
    pkg::build_with(&SEED, KX_BUILD, "k", &bootimg_dt(2, 7), &[("unit", unit), kexec::LINE]).unwrap()
}

#[test]
fn a_package_for_another_unit_is_refused_before_writes() {
    let f = Fake::new("unit-refuse", Some(1));
    let before = (f.region(Region::Recovery), f.region(Region::Slot), f.magic(), f.st());
    let result = install(&mut f.card(), &f.state(), &unit_package("other-unit"), &[pk()], &UNIT_RUN);
    assert!(result.is_err(), "a foreign unit package was accepted: {:?}", result);
    assert!(result.unwrap_err().contains("unit"));
    assert_eq!(before, (f.region(Region::Recovery), f.region(Region::Slot), f.magic(), f.st()));
}

#[test]
fn a_package_for_another_unit_is_refused_before_kexec_staging() {
    let f = Fake::new("unit-stage", Some(1));
    let result = kexec_stage(
        &mut f.card(),
        &f.state(),
        &unit_package("other-unit"),
        &[pk()],
        &UNIT_RUN,
        Some(&tree(7)),
        None,
        None,
    );
    assert!(result.is_err(), "a foreign unit package was staged");
    assert_eq!(f.st().kexec, None);
    assert_eq!(f.region(Region::Recovery), bootimg(1));
}

#[test]
fn override_allows_the_install_and_logs_both_units() {
    let f = Fake::new("unit-override", Some(1));
    let opt = InstallOpt { any_unit: true, ..UNIT_RUN };
    let r = install(&mut f.card(), &f.state(), &unit_package("other-unit"), &[pk()], &opt).unwrap();
    assert!(r.written);
    assert!(r
        .lines
        .iter()
        .any(|l| l.contains("--any-unit override") && l.contains("other-unit") && l.contains("fake-unit")));
}

#[test]
fn unit_identity_is_independent_of_a_changed_device_tree() {
    let p = pkg::build_with(&SEED, KX_BUILD, "k", &bootimg(2), &[("unit", "fake-unit"), kexec::LINE]).unwrap();
    let h = pkg::verify_header(&p, &[pk()]).unwrap();
    assert!(identity::check(&h.manifest, "fake-unit", false).unwrap().is_none());
    let f = Fake::new("unit-different-tree", None);
    let old = rsce(&dtb(&[("model", b"DHO\0")], vec![chosen(b"a\0")]));
    let new = rsce(&dtb(&[("model", b"DHO\0"), ("gpu", b"okay\0")], vec![chosen(b"a\0")]));
    let running = dtb(&[("model", b"DHO\0")], vec![chosen(b"a loader\0")]);
    let p =
        pkg::build_with(&SEED, KX_BUILD, "k", &bootimg_second(2, new), &[("unit", "fake-unit"), kexec::LINE]).unwrap();
    let (r, staged) =
        kexec_stage(&mut f.card(), &f.state(), &p, &[pk()], &UNIT_RUN, Some(&old), None, Some(&running)).unwrap();
    assert!(staged.unwrap().tree.is_some());
    assert!(r.lines.iter().any(|l| l.contains("another one")));
}

#[test]
fn unit_to_none_and_old_packages_require_an_override() {
    for extra in [vec![], vec![("unit", "none")], vec![("unit", "another-unit")]] {
        let p = pkg::build_with(&SEED, KX_BUILD, "k", &bootimg(2), &extra).unwrap();
        let h = pkg::verify_header(&p, &[pk()]).unwrap();
        let e = identity::check(&h.manifest, "test-unit", false).unwrap_err();
        assert!(e.contains("test-unit") && e.contains("--any-unit"));
        let log = identity::check(&h.manifest, "test-unit", true).unwrap().unwrap();
        assert!(log.contains("override") && log.contains("test-unit"));
    }
}

#[test]
fn old_packages_keep_their_signed_bytes_and_default_to_none() {
    let p = package(2, KX_BUILD);
    let h = pkg::verify_header(&p, &[pk()]).unwrap();
    assert_eq!(identity::package(&h.manifest).unwrap(), "none");
    assert!(!h.manifest.signed_text().contains("unit "));
    assert!(identity::check(&h.manifest, "none", false).unwrap().is_none());
}

#[test]
fn unit_metadata_is_signed_and_cannot_be_ambiguous() {
    let p = unit_package("other-unit");
    let h = pkg::verify_header(&p, &[pk()]).unwrap();
    assert!(identity::check(&h.manifest, "other-unit", false).unwrap().is_none());
    let mut bad = p.clone();
    let at = bad.windows(10).position(|w| w == b"other-unit").unwrap();
    bad[at] = b'O';
    assert!(pkg::verify(&bad, &[pk()]).unwrap_err().contains("SIGNATURE INVALID"));
    for extra in [vec![("unit", "")], vec![("unit", "a b")], vec![("unit", "none"), ("unit", "other-unit")]] {
        let p = pkg::build_with(&SEED, KX_BUILD, "k", &bootimg(2), &extra).unwrap();
        let h = pkg::verify_header(&p, &[pk()]).unwrap();
        assert!(identity::check(&h.manifest, "none", true).is_err());
    }
    let f = Fake::new("unit-bad-sig", Some(1));
    assert!(install(&mut f.card(), &f.state(), &bad, &[pk()], &InstallOpt { any_unit: true, ..UNIT_RUN })
        .unwrap_err()
        .contains("SIGNATURE INVALID"));
    assert_eq!(f.region(Region::Recovery), bootimg(1));
}

#[test]
fn running_identity_defaults_only_when_missing() {
    let f = Fake::new("unit-file", None);
    let path = f.dir.join("unit");
    assert_eq!(identity::read(&path).unwrap(), "none");
    std::fs::write(&path, "test-unit\n").unwrap();
    assert_eq!(identity::read(&path).unwrap(), "test-unit");
    std::fs::write(&path, "\n").unwrap();
    assert!(identity::read(&path).is_err());
    assert!(identity::read(&f.dir).is_err());
    for name in ["../x", "/x", "two words", "", &"x".repeat(65)] {
        assert!(identity::validate(name).is_err());
    }
}

#[test]
fn kexec_override_is_logged_before_staging_and_leaves_recovery_intact() {
    let f = Fake::new("unit-override-kexec", Some(1));
    let opt = InstallOpt { any_unit: true, ..UNIT_RUN };
    let (r, staged) =
        kexec_stage(&mut f.card(), &f.state(), &unit_package("other-unit"), &[pk()], &opt, Some(&tree(7)), None, None)
            .unwrap();
    assert!(staged.is_some());
    assert!(r.lines.iter().any(|l| l.contains("--any-unit override") && l.contains("other-unit")));
    assert_eq!(f.region(Region::Recovery), bootimg(1));
    assert_eq!(f.st().kexec.as_deref(), Some(KX_BUILD));
}

#[test]
fn explicit_unit_controls_install_and_stage_without_host_identity() {
    for unit in ["fake-unit", "none"] {
        let f = Fake::new(&format!("unit-explicit-{}", unit), Some(1));
        let opt = InstallOpt { running_unit: unit, ..RUN };
        let p = unit_package(unit);
        let (_, staged) =
            kexec_stage(&mut f.card(), &f.state(), &p, &[pk()], &opt, Some(&tree(7)), None, None).unwrap();
        assert!(staged.is_some());
        let r = install(&mut f.card(), &f.state(), &p, &[pk()], &opt).unwrap();
        assert!(r.written);
        assert!(!r.lines.iter().any(|l| l.contains("override")));
    }
}

#[test]
fn explicit_named_unit_refuses_none_and_old_packages_before_any_writes() {
    for p in [unit_package("none"), package(2, KX_BUILD)] {
        let f = Fake::new("unit-explicit-refuse-none", Some(1));
        let opt = InstallOpt { running_unit: "fake-unit", ..RUN };
        let before = (f.region(Region::Recovery), f.region(Region::Slot), f.magic(), f.st());
        let err = install(&mut f.card(), &f.state(), &p, &[pk()], &opt).unwrap_err();
        assert!(err.contains("package unit 'none', running image unit 'fake-unit'"));
        assert!(kexec_stage(&mut f.card(), &f.state(), &p, &[pk()], &opt, Some(&tree(7)), None, None).is_err());
        assert_eq!(before, (f.region(Region::Recovery), f.region(Region::Slot), f.magic(), f.st()));
    }
}
