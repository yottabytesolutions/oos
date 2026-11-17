// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

#[test]
fn trusted_keys_file_parses() {
    // every line that is not a comment must be a key (a typo would silently drop it)
    let lines = TRUSTED_KEYS.lines().filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#')).count();
    assert_eq!(trusted_keys().len(), lines);
}

#[test]
fn builds_trust_only_the_project_release_key() {
    let owner = hash::unhex("f5f316e2b278b4c29a3724e711493ebb748f7f22d6c9eff265822110f4a4343d").unwrap();
    assert_eq!(trusted_keys(), vec![<[u8; 32]>::try_from(owner).unwrap()]);
    assert!(!trusted_keys().contains(&pk()), "the test signing key must never be trusted by a build");
}

#[test]
fn old_community_signature_is_refused_without_writes() {
    // Public legacy seed, retained only to prove that packages it signs are no longer trusted.
    let seed = hash::unhex("ea37d041219d2ee4a7e283faaf201fcd263d0f2c704953c731658b356e16de12").unwrap();
    let seed: [u8; 32] = seed.try_into().unwrap();
    let key = ed25519::public_key(&seed);
    assert_eq!(hash::hex(&pkg::key_id(&key)), "1d64e92f58d378be");
    let p = pkg::build(&seed, "20261106-000000", "k", &bootimg(2)).unwrap();
    pkg::verify(&p, &[key]).unwrap();
    let f = Fake::new("old-community", Some(1));
    let before = (f.magic(), f.region(Region::Recovery), f.region(Region::Slot));
    let error = install(&mut f.card(), &f.state(), &p, &trusted_keys(), &RUN).unwrap_err();
    assert!(error.contains("unknown key"), "{error}");
    assert_eq!((f.magic(), f.region(Region::Recovery), f.region(Region::Slot)), before);
    assert!(!f.state().exists());
}
