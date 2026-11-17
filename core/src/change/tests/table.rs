// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

#[test]
fn every_table_entry_captures_applies_and_reuses_its_field() {
    let before = Model { trig_type: TrigType::Bus, ..Model::default() };
    flat::assert_exhaustive(&before);
    let mut indexes = Vec::new();
    for &(index, name) in flat::ENTRIES {
        let &(_, mutate, _) = contracts::all().find(|&&(field, _, _)| field == name).expect("every row has a fixture");
        let mut after = before.clone();
        mutate(&mut after);
        assert_ne!(before, after, "fixture for {name}");
        let mut patch = Patch::capture(index, &after).unwrap();
        assert_eq!(patch.index(), index, "index of {name}");
        assert_eq!(patch, patch.clone());
        assert_ne!(patch, Patch::capture(index, &before).unwrap());
        let mut applied = before.clone();
        patch.apply(&mut applied);
        assert_eq!(applied, after, "apply {name}");
        patch.reuse(&before);
        patch.apply(&mut applied);
        assert_eq!(applied, before, "restore {name}");
        patch.reuse(&after);
        patch.apply(&mut applied);
        assert_eq!(applied, after, "reuse {name}");
        let mut snapshot = before.clone();
        refresh(&mut snapshot, &after);
        assert_eq!(snapshot, after, "refresh {name}");
        let mut slot = None;
        fields(&before, &after, |f| {
            if f.index() == index {
                assert!(f.changed(), "diff {name}");
                assert_eq!(f.name(), name);
                assert!(slot.replace(f).is_none(), "duplicate slot {name}");
            }
        });
        assert!(slot.is_some(), "missing slot {name}");
        indexes.push(index);
    }
    indexes.sort_unstable();
    assert_eq!(indexes, (0..MEMBER_FIELDS.start).chain(MEMBER_FIELDS.end..FIELD_COUNT).collect::<Vec<_>>());
    assert_eq!(flat::ENTRIES.len(), contracts::all().count());
    for index in MEMBER_FIELDS.chain(FIELD_COUNT..FIELD_COUNT + 3).chain([usize::MAX]) {
        assert!(Patch::capture(index, &before).is_none());
    }
}

fn mutate_sequence(model: &mut Model, seed: &mut u64) {
    let cases: Vec<_> = contracts::all().collect();
    for _ in 0..8 {
        *seed ^= *seed << 13;
        *seed ^= *seed >> 7;
        *seed ^= *seed << 17;
        (cases[*seed as usize % cases.len()].1)(model);
    }
}

fn assert_round_trip(before: &Model, after: &Model, patches: &mut [Patch]) {
    let mut applied = before.clone();
    for patch in patches {
        patch.reuse(after);
        patch.apply(&mut applied);
    }
    assert_eq!(applied, *after);
    let mut snapshot = before.clone();
    refresh(&mut snapshot, after);
    assert_eq!(snapshot, *after);
}

#[test]
fn all_table_entries_round_trip_combinations_and_seeded_sequences() {
    let mut model = Model::default();
    let mut patches: Vec<_> = flat::ENTRIES.iter().map(|&(i, _)| Patch::capture(i, &model).unwrap()).collect();
    for &(_, mutate, _) in contracts::all() {
        let before = model.clone();
        mutate(&mut model);
        assert_round_trip(&before, &model, &mut patches);
    }
    let mut seed = 0x1938_73ab;
    for _ in 0..256 {
        let before = model.clone();
        mutate_sequence(&mut model, &mut seed);
        assert_round_trip(&before, &model, &mut patches);
    }
}

fn heap_model() -> Model {
    let mut model = Model::default();
    for s in model.ch_label.iter_mut().chain(&mut model.math_expr).chain(&mut model.view_slots) {
        *s = "fixture".repeat(64);
    }
    model.bode_addr = "fixture".repeat(64);
    model.meas = vec![(Meas::Freq, WaveId::Ch(0)); 64];
    model.meas2 = vec![(0, WaveId::Ch(0), WaveId::Ch(1)); 64];
    model.cat = vec![(0, WaveId::Ch(0), WaveId::Ch(1), WaveId::Ch(2), WaveId::Ch(3)); 64];
    model.bus.label = "fixture".repeat(64);
    for zone in &mut model.zones {
        zone.on = true;
        zone.src = 1;
        zone.inside = false;
        zone.pts = vec![(1.0, 2.0); 64];
    }
    model
}

fn payload_bytes(patches: &[Patch]) -> usize {
    patches.iter().map(Patch::bytes).sum()
}

#[test]
fn table_storage_accounts_for_payloads_and_retains_warmed_capacity() {
    let large = heap_model();
    let small = Model::default();
    let mut patches: Vec<_> = flat::ENTRIES.iter().map(|&(i, _)| Patch::capture(i, &large).unwrap()).collect();
    let boxed_bytes = std::mem::size_of_val(&large.maths) + std::mem::size_of_val(&large.decs);
    assert_eq!(payload_bytes(&patches), snapshot_bytes(&large) - std::mem::size_of::<Model>() + boxed_bytes);
    let mut snapshot = large.clone();
    for model in [&small, &large, &small, &large] {
        assert_round_trip(&snapshot, model, &mut patches);
        refresh(&mut snapshot, model);
        let bytes = payload_bytes(&patches);
        assert_round_trip(model, model, &mut patches);
        assert_eq!(payload_bytes(&patches), bytes, "warmed patch capacity");
        let bytes = snapshot_bytes(&snapshot);
        refresh(&mut snapshot, model);
        assert_eq!(snapshot_bytes(&snapshot), bytes, "warmed snapshot capacity");
    }
}
