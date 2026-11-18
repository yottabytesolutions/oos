// SPDX-License-Identifier: GPL-2.0-or-later

use super::super::*;
use super::allocation_budget::{CLEAR_ALLOCATIONS, DEFAULT_ALLOCATIONS, WARMUP_STEPS};
use crate::system::test_allocator::count_own;
use oos_core::change::{Classes as C, Requester};

fn base(run: RunState) -> Ui {
    let mut ui = Ui::new(Source::Demo { t: 0.0 }, host::Host::headless());
    ui.m.meas.clear();
    ui.m.meas2.clear();
    ui.m.run = run;
    ui.step(0.04);
    change::reset(&mut ui);
    ui
}

struct OracleCase {
    name: &'static str,
    run: RunState,
    apply: fn(&mut Ui),
    classes: C,
    raw: [u64; 10],
    max_allocations: usize,
}

const ORACLE_CASES: &[OracleCase] = &[
    OracleCase {
        name: "stopped offset, D2",
        run: RunState::Stop,
        apply: |u| {
            u.m.ch[0].offset += 0.25;
            change::edited(u);
        },
        classes: C::ACQ.union(C::MAPPING).union(C::STATS),
        raw: [0, 1, 0, 0, 0, 0, 0, 0, 0, 0],
        max_allocations: DEFAULT_ALLOCATIONS,
    },
    OracleCase {
        name: "running offset",
        run: RunState::Run,
        apply: |u| {
            u.m.ch[0].offset += 0.25;
            change::edited(u);
        },
        classes: C::ACQ.union(C::MAPPING).union(C::STATS),
        raw: [0; 10],
        max_allocations: DEFAULT_ALLOCATIONS,
    },
    OracleCase {
        name: "running brightness, D1",
        run: RunState::Run,
        apply: |u| {
            u.m.grid_bright = if u.m.grid_bright == 80 { 100 } else { 80 };
            change::edited(u);
        },
        classes: C::NONE,
        raw: [0, 1, 0, 1, 0, 0, 0, 0, 0, 0],
        max_allocations: DEFAULT_ALLOCATIONS,
    },
    OracleCase {
        name: "stopped brightness, D1",
        run: RunState::Stop,
        apply: |u| {
            u.m.grid_bright = if u.m.grid_bright == 80 { 100 } else { 80 };
            change::edited(u);
        },
        classes: C::NONE,
        raw: [0, 0, 0, 1, 0, 0, 0, 0, 0, 0],
        max_allocations: DEFAULT_ALLOCATIONS,
    },
    OracleCase {
        name: "same value mark, D1",
        run: RunState::Run,
        apply: change::edited,
        classes: C::NONE,
        raw: [0, 1, 0, 1, 0, 0, 0, 0, 0, 0],
        max_allocations: DEFAULT_ALLOCATIONS,
    },
    OracleCase {
        name: "trigger has no mapping",
        run: RunState::Stop,
        apply: |u| {
            u.m.trig_level += 0.1;
            change::edited(u);
        },
        classes: C::ACQ.union(C::STATS),
        raw: [0; 10],
        max_allocations: DEFAULT_ALLOCATIONS,
    },
    OracleCase {
        name: "unmarked offset clamp, D3",
        run: RunState::Stop,
        apply: |u| u.m.ch[0].offset = 1e6,
        classes: C::ACQ.union(C::MAPPING).union(C::STATS),
        raw: [0, 1, 0, 0, 0, 0, 0, 0, 0, 0],
        max_allocations: DEFAULT_ALLOCATIONS,
    },
    OracleCase {
        name: "CLEAR at equal settings, D4",
        run: RunState::Stop,
        apply: |u| u.key(key::CLEAR),
        classes: C::PHOSPHOR
            .union(C::STATS)
            .union(C::MASK)
            .union(C::ZONE)
            .union(C::HW_AVERAGE)
            .union(C::MATH)
            .union(C::VIEW),
        raw: [0, 0, 0, 0, 0, 0, 1, 0, 1, 1],
        max_allocations: CLEAR_ALLOCATIONS,
    },
];

// Each fixture pins a union from the task oracle, independently of classify. Geometry is included explicitly.
#[test]
fn scripted_change_storm_matches_reviewed_oracle() {
    for case in ORACLE_CASES {
        let name = case.name;
        let classes = case.classes;
        let mut ui = base(case.run);
        ui.mstat = vec![MStat { n: 7, ..Default::default() }];
        (case.apply)(&mut ui);
        assert_eq!(ui.mstat[0].n, 7, "input defers statistics: {name}");
        change::begin_step(&mut ui, 0.04).unwrap();
        assert_eq!(ui.host.runtime().change.last.as_ref().unwrap().classes(), classes, "{name}");
        assert_eq!(ui.mstat.is_empty(), classes.contains(C::STATS), "{name}");
        let (actual, _, outbox, errors, unreviewed) = change::counters(&ui);
        assert_eq!(actual, [0; 10], "retired legacy counters (previously {:?}): {name}", case.raw);
        assert_eq!((outbox, errors, unreviewed), (0, 0, 0), "{name}");
    }
}

#[test]
fn knob_scpi_web_command_and_setup_share_shadow_field_contract() {
    let mut knob = base(RunState::Stop);
    knob.key(KNOB_NAMES.iter().find(|(name, _)| *name == "vpos").unwrap().1);
    let offset = knob.m.ch[0].offset;
    let mut scpi = base(RunState::Stop);
    let mut remote = crate::scpi::Scpi::open(0u16, false);
    remote.exec_line(&mut scpi, &Canvas::new(1, 1), &format!(":CHAN1:OFFS {offset}"));
    // the end of the SCPI message is its change point; the other paths are accepted by their step
    let scpi_classes = scpi.host.runtime().change.last.as_ref().unwrap().classes();
    let mut web = base(RunState::Stop);
    // HTTP /cmd uses this exact parser after authorization; it does not have a separate settings setter.
    crate::cmd::run(&mut web, "knob vpos +1");
    let mut setup = base(RunState::Stop);
    let text = crate::store::setup_text(&knob.m);
    let dir = std::env::temp_dir().join(format!("oos-m22-setup-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    setup.store_dir = dir.clone();
    std::fs::write(setup.setup_path(), text).unwrap();
    setup.recall_setup();
    std::fs::remove_dir_all(dir).unwrap();
    for ui in [&mut knob, &mut web, &mut setup] {
        ui.step(0.04);
    }
    let expected = C::ACQ | C::MAPPING | C::STATS;
    for ui in [&knob, &web, &setup] {
        assert_eq!(ui.m.ch[0].offset, offset);
        assert_eq!(ui.host.runtime().change.last.as_ref().unwrap().classes(), expected);
    }
    assert_eq!(scpi.m.ch[0].offset, offset);
    assert_eq!(scpi_classes, expected);
}

#[test]
fn shadow_is_observe_only() {
    let mut control = base(RunState::Run);
    let mut injected = base(RunState::Run);
    change::inject(&mut injected);
    control.step(0.04);
    injected.step(0.04);
    assert_eq!(control.m, injected.m);
    assert_eq!(control.ph.maps(), injected.ph.maps());
    assert_eq!(control.phz.maps(), injected.phz.maps());
    assert_eq!((control.records, control.wfm_total), (injected.records, injected.wfm_total));
    assert_eq!(control.last[0].as_ref().unwrap().ch, injected.last[0].as_ref().unwrap().ch);
    assert_eq!(control.mstat.len(), injected.mstat.len());
    assert_ne!(control.host.runtime().change.last, injected.host.runtime().change.last);
}

#[test]
fn same_settings_stage_retry_resolves_tokens_in_the_next_shadow_step() {
    let mut ui = base(RunState::Stop);
    change::stage(&mut ui, Requester::Autoset);
    assert!(ui.host.runtime().change.last.is_none());
    ui.step(0.04);
    let first = ui.host.runtime().change.last.as_ref().unwrap().identity();
    change::stage(&mut ui, Requester::Autoset);
    ui.step(0.04);
    let second = ui.host.runtime().change.last.as_ref().unwrap().identity();
    assert_eq!(second.epoch(), first.epoch() + 1);
    assert_eq!(second.gen(), first.gen());
    assert_eq!(change::counters(&ui).2, 0);
}

#[test]
fn shadow_counters_are_available_to_state_queries() {
    let mut ui = base(RunState::Stop);
    ui.m.grid_bright -= 10;
    crate::ui::change::edited(&mut ui);
    ui.step(0.04);
    let text = crate::cmd::stat_text(&ui);
    for name in C::NAMES {
        assert!(text.contains(&format!("{name}=")), "{text}");
    }
    let mut remote = crate::scpi::Scpi::open(0u16, false);
    let reply = remote.exec_line(&mut ui, &Canvas::new(1, 1), ":SYST:STAT?");
    assert!(String::from_utf8(reply).unwrap().contains(" shadow acq="));
}

#[test]
#[ignore = "explicit allocation guard; run with OOS_PERF_TESTS=1"]
fn perf_guard_intake_change_shadow_budget() {
    if std::env::var("OOS_PERF_TESTS").as_deref() != Ok("1") {
        return;
    }
    for case in ORACLE_CASES {
        let name = case.name;
        let limit = case.max_allocations;
        let mut ui = base(case.run);
        let mut maximum = 0;
        // Warm the exact input/change path before checking retained storage reuse.
        for step in 0..100 {
            let (allocations, ()) = count_own(|| {
                change::intake(&mut ui, case.apply);
                change::begin_step(&mut ui, 0.04).unwrap();
                change::finish_step(&mut ui);
            });
            if step >= WARMUP_STEPS {
                maximum = maximum.max(allocations);
            }
        }
        assert!(maximum <= limit, "{name}: intake/change allocations {maximum} exceed {limit}");
    }
}

#[test]
fn autoset_and_bode_propose_settings_and_epochs_without_a_device() {
    for bode in [false, true] {
        // Shared is a fake producer here: no spawn, device access, transport or installed generator.
        let mut ui = Ui::new(Source::Hw { shared: Default::default(), seen: 0 }, host::Host::headless());
        ui.m.run = RunState::Stop;
        ui.m.meas.clear();
        ui.m.bode_gen = super::super::bode::FREE;
        ui.step(0.04);
        change::reset(&mut ui);
        if bode {
            ui.bode_begin();
        } else {
            ui.auto_start();
        }
        ui.step(0.04);
        let result = ui.host.runtime().change.last.as_ref().unwrap();
        let expected =
            C::ACQ | C::MAPPING | C::PHOSPHOR | C::STATS | C::HW_AVERAGE | if bode { C::VIEW } else { C::NONE };
        assert_eq!(result.classes(), expected, "bode={bode}");
        assert!(result.identity().epoch() > 0);
        let (raw, _, outbox, errors, unreviewed) = change::counters(&ui);
        assert_eq!((raw, outbox, errors, unreviewed), ([0; 10], 0, 0, 0), "bode={bode}");
    }
}

#[test]
fn oracle_differences_do_not_exempt_unrelated_classes() {
    use change::oracle::{difference, Row};
    for (row, class) in [
        (Row::Mark, C::ACQ),
        (Row::Normalize, C::MASK),
        (Row::Math, C::STATS),
        (Row::Clear, C::HISTORY),
        (Row::Single, C::VIEW),
        (Row::Run, C::PHOSPHOR),
    ] {
        assert_eq!(difference(row, class, true, change::oracle::OracleContext::default()), None);
        assert_eq!(difference(row, class, false, change::oracle::OracleContext::default()), None);
    }
}

#[test]
fn repeated_requests_keep_exact_oracle_counter_deltas() {
    let mut run = base(RunState::Stop);
    run.key(key::RUNSTOP);
    run.step(0.04);
    let first = run.host.runtime().change.last.as_ref().unwrap().identity();
    assert_eq!(run.host.runtime().change.last.as_ref().unwrap().classes(), C::HW_AVERAGE);
    assert_eq!(change::counters(&run).0, [0; 10]);
    run.key(key::RUNSTOP);
    run.step(0.04);
    assert_eq!(run.host.runtime().change.last.as_ref().unwrap().classes(), C::NONE);
    assert_eq!(run.host.runtime().change.last.as_ref().unwrap().identity().epoch(), first.epoch());
    let mut single = base(RunState::Stop);
    single.key(key::SINGLE);
    single.step(0.04);
    let first = single.host.runtime().change.last.as_ref().unwrap().identity();
    assert_eq!(single.host.runtime().change.last.as_ref().unwrap().classes(), C::PHOSPHOR | C::STATS | C::HW_AVERAGE);
    assert_eq!(change::counters(&single).0, [0; 10]);
    single.key(key::SINGLE);
    single.step(0.04);
    assert_eq!(single.host.runtime().change.last.as_ref().unwrap().identity().epoch(), first.epoch() + 1);
    assert_eq!(change::counters(&single).0, [0; 10]);
    assert_eq!(change::counters(&single).4, 0);
    let mut mask = base(RunState::Stop);
    mask.create_mask();
    mask.step(0.04);
    change::reset(&mut mask);
    mask.create_mask();
    mask.step(0.04);
    assert_eq!(mask.host.runtime().change.last.as_ref().unwrap().classes(), C::MASK);
    assert_eq!(change::counters(&mask), ([0; 10], 0, 0, 0, 0));
}

#[test]
fn a_late_stage_resolves_its_settings_and_token_together_next_step() {
    let mut ui = base(RunState::Run);
    ui.step(0.04);
    let old = ui.host.runtime().change.last.as_ref().unwrap().identity();
    let token = change::late_stage(&mut ui);
    let current = ui.host.runtime().change.last.as_ref().unwrap();
    assert_eq!(current.identity(), old, "the earlier change point cannot include a later stage");
    assert_eq!(current.epoch(token), None);
    ui.step(0.04);
    let next = ui.host.runtime().change.last.as_ref().unwrap();
    assert_eq!(next.identity().gen(), old.gen() + 1);
    assert_eq!(next.identity().epoch(), old.epoch() + 1);
    assert_eq!(next.epoch(token), Some(next.identity().epoch()));
    assert_eq!(change::counters(&ui).2, 0);
}

#[test]
fn same_final_run_value_keeps_the_intake_epoch_request() {
    let mut ui = base(RunState::Run);
    ui.key(key::RUNSTOP);
    ui.key(key::RUNSTOP);
    ui.step(0.04);
    let changes = ui.host.runtime().change.last.as_ref().unwrap();
    assert_eq!(changes.classes(), C::HW_AVERAGE);
    assert_eq!(changes.identity().epoch(), 1);
    assert_eq!(change::counters(&ui).0, [0; 10]);
    assert_eq!(change::counters(&ui).4, 0);
}

#[test]
fn rejected_shadow_requests_are_quarantined() {
    let mut ui = base(RunState::Stop);
    assert_eq!(change::duplicate_requests(&mut ui), 2);
    ui.step(0.04);
    assert_eq!(change::pending_count(&ui), 0);
    assert_eq!(change::counters(&ui).3, 1);
    ui.step(0.04);
    assert_eq!(change::pending_count(&ui), 0);
    assert_eq!(change::counters(&ui).3, 1);
    assert!(change::text(&ui).contains("DuplicateToken=1"));
}

#[test]
fn explicit_measurement_reset_is_a_request_at_equal_settings() {
    let mut ui = base(RunState::Stop);
    for _ in 0..2 {
        ui.mstat = vec![MStat { n: 7, ..Default::default() }];
        change::stats_reset(&mut ui);
        ui.step(0.04);
        assert_eq!(ui.host.runtime().change.last.as_ref().unwrap().classes(), C::STATS);
        assert!(ui.mstat.is_empty());
    }
    assert_eq!(change::counters(&ui), ([0; 10], 0, 0, 0, 0));
}

#[test]
fn stopped_scpi_mark_keeps_its_oracle_row_after_a_panel_run() {
    let mut ui = base(RunState::Stop);
    let mut remote = crate::scpi::Scpi::open(0u16, false);
    remote.exec_line(&mut ui, &Canvas::new(1, 1), ":CHAN1:OFFS 0.25");
    // the end of the SCPI message is its own change point; the panel RUN is the step's
    let scpi = ui.host.runtime().change.last.as_ref().unwrap().classes();
    assert_eq!(scpi, C::ACQ | C::MAPPING | C::STATS);
    ui.key(key::RUNSTOP);
    ui.step(0.04);
    let panel = ui.host.runtime().change.last.as_ref().unwrap().classes();
    assert_eq!(scpi | panel, C::ACQ | C::MAPPING | C::STATS | C::HW_AVERAGE);
    assert_eq!(change::counters(&ui), ([0; 10], 0, 0, 0, 0));
}

#[test]
fn zoom_clamp_has_a_producing_oracle_row() {
    let mut ui = base(RunState::Stop);
    ui.m.zscale = ui.m.tscale.saturating_sub(2);
    ui.step(0.04);
    change::reset(&mut ui);
    ui.m.zdelay = 1e3;
    ui.step(0.04);
    assert!(ui.host.runtime().change.last.as_ref().unwrap().classes().contains(C::MAPPING));
    assert_eq!(change::counters(&ui), ([0; 10], 0, 0, 0, 0));
}

#[test]
fn run_request_classes_preserve_the_source_specific_d4_difference() {
    for hardware in [false, true] {
        // Fake Shared has no producer thread or device access.
        let source =
            if hardware { Source::Hw { shared: Default::default(), seen: 0 } } else { Source::Demo { t: 0.0 } };
        let mut ui = Ui::new(source, host::Host::headless());
        ui.m.run = RunState::Stop;
        ui.m.meas.clear();
        ui.m.meas2.clear();
        ui.step(0.04);
        change::reset(&mut ui);
        let before = ui.m.hw_avg_clear;
        ui.key(key::RUNSTOP);
        ui.step(0.04);
        let changes = ui.host.runtime().change.last.as_ref().unwrap();
        assert_eq!(changes.classes(), C::HW_AVERAGE, "hardware={hardware}");
        assert_eq!(changes.identity().epoch(), 1);
        assert_eq!(changes.identity().gen(), 0, "RUN alone does not reconfigure acquisition");
        assert_eq!(ui.m.hw_avg_clear, before + u64::from(hardware));
        assert_eq!(change::counters(&ui), ([0; 10], 0, 0, 0, 0), "hardware={hardware}");
    }
}

#[test]
fn average_restart_observes_only_hardware_effects() {
    let mut demo = base(RunState::Stop);
    change::run_request(&mut demo);
    demo.step(0.04);
    assert_eq!(change::counters(&demo), ([0; 10], 0, 0, 0, 0));
    let mut hw = Ui::new(Source::Hw { shared: Default::default(), seen: 0 }, host::Host::headless());
    hw.m.run = RunState::Stop;
    hw.m.meas.clear();
    hw.step(0.04);
    change::reset(&mut hw);
    let before = hw.m.hw_avg_clear;
    change::run_request(&mut hw);
    hw.step(0.04);
    assert_eq!(hw.m.hw_avg_clear, before + 1);
    assert_eq!(change::counters(&hw).0, [0; 10]);
}

#[test]
fn intake_guard_balances_nested_errors_and_unwinds() {
    let mut ui = base(RunState::Stop);
    let error: Result<(), ()> = change::intake(&mut ui, |ui| change::intake(ui, |_| Err(())));
    assert_eq!(error, Err(()));
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        change::intake(&mut ui, |ui| change::intake(ui, |_| panic!("test intake unwind")));
    }));
    assert!(panic.is_err());
    change::intake(&mut ui, |_| ());
    assert!(change::timing(&ui).intake_idle());
}
