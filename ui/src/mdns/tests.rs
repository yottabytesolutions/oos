// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;

#[test]
fn names_of_the_scope() {
    let serial = concat!("DHO", "9A264204527");
    for s in ["dho924s-204527", "a", "lab-3", &"x".repeat(63)] {
        assert!(name_ok(s), "{}", s);
    }
    for s in ["", "-a", "a-", "a b", "a.b", "ä", &"x".repeat(64), "a_b"] {
        assert!(!name_ok(s), "{}", s);
    }
    assert_eq!(default_name("DHO924S", Some(serial)), "dho924s-204527");
    assert_eq!(default_name("DHO804", Some(concat!("DHO", "8A000000001"))), "dho804-000001");
    assert_eq!(default_name("DHO924S", None), "dho924s");
    assert_eq!(default_name("DHO924S", Some("ABCDEFGH")), "dho924s-cdefgh");
    assert_eq!(default_name("", None), "oos");
    assert_eq!(default_name("DHO 924/S", Some(" 12 ")), "dho924s-12");
    assert!(name_ok(&default_name(&"M".repeat(80), Some("1234567"))));
    assert_eq!(instance_name("DHO924S", Some(serial), "dho924s-204527"), "Rigol DHO924S OOS (204527)");
    assert_eq!(instance_name("DHO924S", Some(serial), "lab-scope-3"), "Rigol DHO924S OOS (lab-scope-3)");
    assert_eq!(instance_name("DHO924S", None, "dho924s"), "Rigol DHO924S OOS (dho924s)");
    assert!(instance_name("DHO924S", None, &"x".repeat(63)).len() <= NAME_MAX);
}
