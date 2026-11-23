// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::system::test_allocator::count_own;
use crate::ui::tests::allocation_budget::{DEFAULT_ALLOCATIONS, WARMUP_STEPS};

fn running() -> Ui {
    let mut ui = Ui::new(Source::Hw { shared: Default::default(), seen: 0 }, host::Host::headless());
    ui.m.meas.clear();
    ui.m.meas2.clear();
    ui.m.persist = PERSIST.len() - 1;
    ui.step(0.04);
    change::reset(&mut ui);
    ui
}

fn seed(ui: &mut Ui) {
    for ph in [&mut ui.ph, &mut ui.phz] {
        for c in 0..NCH {
            ph.channel_mut(c)[0] = 97;
        }
    }
}

fn empty(ui: &Ui) -> bool {
    [&ui.ph, &ui.phz].iter().all(|p| p.maps().iter().all(|m| m.iter().all(|&v| v == 0)))
}

#[test]
fn mapping_and_phosphor_clear_at_the_change_point_without_a_mark() {
    for edit in 0..5 {
        let mut ui = running();
        seed(&mut ui);
        match edit {
            0 => ui.m.delay += ui.m.time_per_div(),
            1 => ui.m.ch[0].offset += 0.1,
            2 => ui.m.persist = 1,
            3 => ui.m.color_grade = !ui.m.color_grade,
            _ => ui.m.avg_count *= 2,
        }
        change::begin_step(&mut ui, 0.04).unwrap();
        assert!(empty(&ui), "both maps must clear at acceptance: edit={edit}");
        seed(&mut ui);
        change::begin_step(&mut ui, 0.04).unwrap();
        assert!(!empty(&ui), "same settings must not clear twice");
    }
}

#[test]
fn unrelated_edits_preserve_picture_and_only_trigger_resets_statistics() {
    for edit in 0..3 {
        let mut ui = running();
        seed(&mut ui);
        ui.mstat = vec![MStat { n: 9, ..Default::default() }];
        match edit {
            0 => ui.m.grid = Grid::None,
            1 => ui.m.trig_level += 0.1,
            _ => {}
        }
        crate::ui::change::edited(&mut ui);
        ui.step(0.04);
        assert_eq!(ui.ph.maps()[0][0], 97, "broad marks no longer erase the picture: {edit}");
        assert_eq!(ui.phz.maps()[0][0], 97);
        assert_eq!(ui.mstat.is_empty(), edit == 1, "only STATS edits clear statistics");
    }
}

#[test]
fn persistence_knob_defers_both_maps_to_acceptance() {
    let mut ui = running();
    seed(&mut ui);
    ui.adjust(Item::Persist, -1);
    assert_eq!(ui.ph.maps()[0][0], 97, "input cannot clear ahead of acceptance");
    assert_eq!(ui.phz.maps()[0][0], 97);
    change::begin_step(&mut ui, 0.04).unwrap();
    assert!(empty(&ui));
}

#[test]
fn min_replaces_only_with_a_new_record_and_decay_still_runs() {
    let mut ui = running();
    ui.m.persist = 0;
    ui.step(0.04);
    seed(&mut ui);
    ui.step(0.04);
    assert_eq!(ui.ph.maps()[0][0], 97, "Min without new data must retain the picture");
    ui.m.persist = 1;
    ui.step(0.04);
    seed(&mut ui);
    let mut want = ui.ph.maps()[0][0];
    let factor = (-0.04 / PERSIST[1].1 as f64).exp() as f32;
    want = ((want as u32 * (factor * 65536.0) as u32) >> 16) as u16;
    ui.step(0.04);
    assert_eq!(ui.ph.maps()[0][0], want);
    assert_eq!(ui.phz.maps()[0][0], want);
}

#[test]
fn clear_and_single_clear_once_per_accepted_request_even_at_equal_settings() {
    for single in [false, true] {
        let mut ui = running();
        for _ in 0..2 {
            seed(&mut ui);
            if single {
                ui.key(key::SINGLE);
            } else {
                ui.clear_display();
            }
            assert_eq!(ui.ph.maps()[0][0], 97, "intake must defer picture effects");
            let (_, cleared) = change::begin_step(&mut ui, 0.04).unwrap();
            assert!(cleared && empty(&ui), "same-value request still acts");
            seed(&mut ui);
            let (_, cleared) = change::begin_step(&mut ui, 0.04).unwrap();
            assert!(!cleared && !empty(&ui), "accepted request is consumed once");
        }
    }
}

#[test]
fn mapping_phosphor_and_their_union_preserve_remaining_consumers() {
    for geometry in [false, true] {
        let mut ui = running();
        seed(&mut ui);
        ui.mstat = vec![MStat { n: 7, ..Default::default() }];
        (ui.mask_pass, ui.mask_fail, ui.zone_drops) = (11, 12, 13);
        ui.m.persist -= 1;
        if geometry {
            ui.m.delay += ui.m.time_per_div();
        }
        let (_, cleared) = change::begin_step(&mut ui, 0.04).unwrap();
        assert!(cleared && empty(&ui));
        let classes = ui.host.runtime().change.last.as_ref().unwrap().classes();
        assert_eq!(classes, C::PHOSPHOR | if geometry { C::MAPPING | C::ACQ | C::STATS } else { C::NONE });
        assert_eq!((ui.mask_pass, ui.mask_fail, ui.zone_drops), (11, 12, 13));
        assert_eq!(ui.mstat.is_empty(), geometry, "STATS follows the effective union");
    }
}

#[test]
fn rejected_mapping_waits_for_acceptance_without_losing_the_clear() {
    let mut ui = running();
    seed(&mut ui);
    ui.m.delay += ui.m.time_per_div();
    ui.m.math_expr[0] = "x".repeat(70_000);
    let (_, cleared) = change::begin_step(&mut ui, 0.04).unwrap();
    assert!(!cleared && !empty(&ui), "a rejected snapshot must not act");
    ui.m.math_expr[0] = String::new();
    let (_, cleared) = change::begin_step(&mut ui, 0.04).unwrap();
    assert!(cleared && empty(&ui), "the effective mapping still needs one accepted clear");
}

#[test]
fn same_dots_and_vertical_zoom_use_effective_geometry() {
    let mut ui = running();
    seed(&mut ui);
    ui.set_dots(ui.m.dots);
    let (_, cleared) = change::begin_step(&mut ui, 0.04).unwrap();
    assert!(!cleared && !empty(&ui));
    ui.zoom_vfactor *= 2.0;
    let (_, cleared) = change::begin_step(&mut ui, 0.04).unwrap();
    assert!(cleared && empty(&ui));
    seed(&mut ui);
    ui.zoom_vcenter += 0.5;
    let (_, cleared) = change::begin_step(&mut ui, 0.04).unwrap();
    assert!(cleared && empty(&ui));
}

#[test]
fn z_input_clears_running_infinite_persistence_on_the_record_route() {
    let mut ui = running();
    seed(&mut ui);
    ui.adjust(Item::ZInput, 1);
    ui.step(0.04);
    assert!(empty(&ui), "new Z input must discard the old picture");
    assert!(ui.host.runtime().change.last.as_ref().unwrap().classes().contains(C::MAPPING));
    assert_eq!(change::counters(&ui).4, 0);
    seed(&mut ui);
    ui.step(0.04);
    assert!(!empty(&ui), "same Z input must retain infinite persistence");
}

#[test]
#[ignore = "explicit allocation guard; run with OOS_PERF_TESTS=1"]
fn perf_guard_picture_consumer_mapping_storm_budget() {
    if std::env::var("OOS_PERF_TESTS").as_deref() != Ok("1") {
        return;
    }
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let mut ui = running();
    ui.set_zoom(true);
    ui.step(0.04);
    change::reset(&mut ui);
    for i in 0..300 {
        seed(&mut ui);
        ui.m.ch[0].offset = (1 + i % 2) as f32 * 0.1;

        let (allocations, (_, cleared)) = count_own(|| change::begin_step(&mut ui, 0.04).unwrap());
        assert!(cleared, "each mapping edit must clear the picture");
        if i >= WARMUP_STEPS {
            assert!(
                allocations <= DEFAULT_ALLOCATIONS,
                "mapping acceptance and picture allocations {allocations} at step {i}"
            );
        }
        ui.step(0.04);
        assert!(empty(&ui), "every edit exercises both map clears");
    }
    assert_eq!(change::counters(&ui), ([0; 10], 0, 0, 0, 0));
}
