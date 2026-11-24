// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::update::models as identity;

const UNIT_RUN: InstallOpt<'static> = InstallOpt { running_model: "fake-unit", ..RUN };

fn unit_package(unit: &str) -> Vec<u8> {
    pkg::build_with(&SEED, KX_BUILD, "k", &bootimg_dt(2, 7), &[("models", unit), kexec::LINE]).unwrap()
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
        BootTrees::new(Some(&tree(7)), None, None),
    );
    assert!(result.is_err(), "a foreign unit package was staged");
    assert_eq!(f.st().kexec, None);
    assert_eq!(f.region(Region::Recovery), bootimg(1));
}

#[test]
fn override_allows_the_install_and_logs_both_units() {
    let f = Fake::new("unit-override", Some(1));
    let opt = InstallOpt { any_model: true, ..UNIT_RUN };
    let r = install(&mut f.card(), &f.state(), &unit_package("other-unit"), &[pk()], &opt).unwrap();
    assert!(r.written);
    assert!(r
        .lines
        .iter()
        .any(|l| l.contains("--any-model override") && l.contains("other-unit") && l.contains("fake-unit")));
}

#[test]
fn unit_identity_is_independent_of_a_changed_device_tree() {
    let p = pkg::build_with(&SEED, KX_BUILD, "k", &bootimg(2), &[("models", "fake-unit"), kexec::LINE]).unwrap();
    let h = pkg::verify_header(&p, &[pk()]).unwrap();
    assert!(identity::check(&h.manifest, "fake-unit", false).unwrap().is_none());
    let f = Fake::new("unit-different-tree", None);
    let old = rsce(&dtb(&[("model", b"DHO\0")], vec![chosen(b"a\0")]));
    let new = rsce(&dtb(&[("model", b"DHO\0"), ("gpu", b"okay\0")], vec![chosen(b"a\0")]));
    let running = dtb(&[("model", b"DHO\0")], vec![chosen(b"a loader\0")]);
    let p = pkg::build_with(&SEED, KX_BUILD, "k", &bootimg_second(2, new), &[("models", "fake-unit"), kexec::LINE])
        .unwrap();
    let (r, staged) = kexec_stage(
        &mut f.card(),
        &f.state(),
        &p,
        &[pk()],
        &UNIT_RUN,
        BootTrees::new(Some(&old), None, Some(&running)),
    )
    .unwrap();
    assert!(staged.unwrap().tree.is_some());
    assert!(r.lines.iter().any(|l| l.contains("another one")));
}

#[test]
fn unit_to_none_and_old_packages_require_an_override() {
    for extra in [vec![], vec![("models", "none")], vec![("models", "another-unit")]] {
        let p = pkg::build_with(&SEED, KX_BUILD, "k", &bootimg(2), &extra).unwrap();
        let h = pkg::verify_header(&p, &[pk()]).unwrap();
        let e = identity::check(&h.manifest, "test-unit", false).unwrap_err();
        assert!(e.contains("test-unit") && e.contains("--any-model"));
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
    for extra in [vec![("models", "")], vec![("models", "a  b")], vec![("models", "none"), ("models", "other-unit")]] {
        let p = pkg::build_with(&SEED, KX_BUILD, "k", &bootimg(2), &extra).unwrap();
        let h = pkg::verify_header(&p, &[pk()]).unwrap();
        assert!(identity::check(&h.manifest, "none", true).is_err());
    }
    let f = Fake::new("unit-bad-sig", Some(1));
    assert!(install(&mut f.card(), &f.state(), &bad, &[pk()], &InstallOpt { any_model: true, ..UNIT_RUN })
        .unwrap_err()
        .contains("SIGNATURE INVALID"));
    assert_eq!(f.region(Region::Recovery), bootimg(1));
}

#[test]
fn unit_named_install_detects_hardware_and_accepts_generic_image() {
    let f = Fake::new("generic-migration", Some(1));
    std::fs::write(f.dir.join("oos-unit"), "old-unit\n").unwrap();
    let conf = b"# Existing device overrides\neth.mac = 00:00:5e:00:53:04\n";
    let secret = b"0123:4567:89ab:cdef:0123:4567:89ab:cdef\n";
    std::fs::write(f.dir.join("system.conf"), conf).unwrap();
    std::fs::create_dir(f.dir.join("net")).unwrap();
    std::fs::write(f.dir.join("net/ipv6.secret"), secret).unwrap();
    let vendor = f.dir.join("vendor.bin");
    std::fs::write(&vendor, crate::model::encode_vendor(&[(5, "DHO804")])).unwrap();
    let running = identity::running_from(&vendor).unwrap();
    assert_eq!(running, "DHO804");
    let names = include_str!("../../../../rootfs/board/dho/etc/oos-models").trim();
    let p = pkg::build_with(&SEED, KX_BUILD, "k", &bootimg(2), &[("models", names)]).unwrap();
    let report =
        install(&mut f.card(), &f.state(), &p, &[pk()], &InstallOpt { running_model: &running, ..RUN }).unwrap();
    assert!(report.written && !report.lines.iter().any(|l| l.contains("override")));
    assert_eq!(std::fs::read_to_string(f.dir.join("oos-unit")).unwrap(), "old-unit\n");
    assert_eq!(std::fs::read(f.dir.join("system.conf")).unwrap(), conf);
    assert_eq!(std::fs::read(f.dir.join("net/ipv6.secret")).unwrap(), secret);
    std::fs::write(&vendor, crate::model::encode_vendor(&[(5, "MHO900")])).unwrap();
    assert!(identity::running_from(&vendor).is_err());
    assert!(identity::running_from(&f.dir.join("missing")).is_err());
}

#[test]
fn generic_image_accepts_every_supported_model_despite_old_unit_metadata() {
    let names = include_str!("../../../../rootfs/board/dho/etc/oos-models").trim();
    let p = pkg::build_with(&SEED, KX_BUILD, "k", &bootimg(2), &[("models", names), ("unit", "old-unit")]).unwrap();
    let h = pkg::verify_header(&p, &[pk()]).unwrap();
    for (name, ..) in crate::dho::target::MODEL_TABLE.iter().filter(|(n, ..)| *n != "DHO900") {
        assert!(names.split(' ').any(|n| n == *name));
    }
    for name in names.split(' ') {
        assert!(crate::model::lookup(name).is_some());
        assert!(identity::check(&h.manifest, name, false).unwrap().is_none());
    }
    assert!(identity::check(&h.manifest, "MHO900", false).is_err());
    for name in ["../x", "/x", "two words", "", &"x".repeat(65)] {
        assert!(identity::validate(name).is_err());
    }
}

#[test]
fn kexec_override_is_logged_before_staging_and_leaves_recovery_intact() {
    let f = Fake::new("unit-override-kexec", Some(1));
    let opt = InstallOpt { any_model: true, ..UNIT_RUN };
    let (r, staged) = kexec_stage(
        &mut f.card(),
        &f.state(),
        &unit_package("other-unit"),
        &[pk()],
        &opt,
        BootTrees::new(Some(&tree(7)), None, None),
    )
    .unwrap();
    assert!(staged.is_some());
    assert!(r.lines.iter().any(|l| l.contains("--any-model override") && l.contains("other-unit")));
    assert_eq!(f.region(Region::Recovery), bootimg(1));
    assert_eq!(f.st().kexec.as_deref(), Some(KX_BUILD));
}

#[test]
fn explicit_unit_controls_install_and_stage_without_host_identity() {
    for unit in ["fake-unit", "none"] {
        let f = Fake::new(&format!("unit-explicit-{}", unit), Some(1));
        let opt = InstallOpt { running_model: unit, ..RUN };
        let p = unit_package(unit);
        let (_, staged) =
            kexec_stage(&mut f.card(), &f.state(), &p, &[pk()], &opt, BootTrees::new(Some(&tree(7)), None, None))
                .unwrap();
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
        let opt = InstallOpt { running_model: "fake-unit", ..RUN };
        let before = (f.region(Region::Recovery), f.region(Region::Slot), f.magic(), f.st());
        let err = install(&mut f.card(), &f.state(), &p, &[pk()], &opt).unwrap_err();
        assert!(err.contains("package models 'none', running device model 'fake-unit'"));
        assert!(kexec_stage(&mut f.card(), &f.state(), &p, &[pk()], &opt, BootTrees::new(Some(&tree(7)), None, None))
            .is_err());
        assert_eq!(before, (f.region(Region::Recovery), f.region(Region::Slot), f.magic(), f.st()));
    }
}
