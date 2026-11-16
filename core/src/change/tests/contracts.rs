// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use Classes as C;
mod acquire;
mod analysis0;
mod analysis1;
mod analysis2;
mod analysis3;
mod channels;
mod display;
mod interfaces;
mod measure;
mod timebase;
mod trigger;
type Case = (&'static str, fn(&mut Model), C);

fn all() -> impl Iterator<Item = &'static Case> {
    [
        acquire::CASES,
        analysis0::CASES,
        analysis1::CASES,
        analysis2::CASES,
        analysis3::CASES,
        channels::CASES,
        display::CASES,
        interfaces::CASES,
        measure::CASES,
        timebase::CASES,
        trigger::CASES,
    ]
    .into_iter()
    .flatten()
}
pub(super) fn unrelated() -> Vec<&'static str> {
    let mut names: Vec<_> = all()
        .filter_map(|&(name, _, c)| {
            (c.is_empty() && !matches!(name, "run" | "status" | "hw_stamp" | "hw_avg_clear")).then_some(name)
        })
        .collect();
    names.sort_unstable();
    names
}

#[test]
fn every_field_contract_and_mapping_key_matches_mapping_fields() {
    let mut tested = 0;
    for cases in [
        acquire::CASES,
        analysis0::CASES,
        analysis1::CASES,
        analysis2::CASES,
        analysis3::CASES,
        channels::CASES,
        display::CASES,
        interfaces::CASES,
        measure::CASES,
        timebase::CASES,
        trigger::CASES,
    ] {
        for (name, mutate, expected) in cases {
            let a = Model { trig_type: TrigType::Bus, ..Model::default() };
            let mut b = a.clone();
            mutate(&mut b);
            assert_ne!(a, b, "fixture did not change {name}");
            let c = classify(&a, &b, UiAcq::default(), UiAcq::default());
            assert_eq!(c, *expected, "field {name}");
            assert_eq!(
                MappingKey::of(&a, Layout::default()) != MappingKey::of(&b, Layout::default()),
                expected.contains(C::MAPPING),
                "mapping field {name}"
            );
            let mut snapshot = a.clone();
            refresh(&mut snapshot, &b);
            assert_eq!(snapshot, b, "snapshot field {name}");
            let mut outbox = Outbox::default();
            outbox.capture(&a, &b).unwrap();
            if !matches!(*name, "status" | "hw_stamp" | "hw_avg_clear") {
                let mut proposed = a.clone();
                outbox.apply(&mut proposed);
                assert_eq!(proposed, b, "typed proposal field {name}");
            } else {
                assert!(outbox.entries().is_empty());
            }
            tested += 1;
        }
    }
    assert_eq!(tested, 195);
}
