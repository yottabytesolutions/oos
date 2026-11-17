// SPDX-License-Identifier: GPL-2.0-or-later

use super::super::*;
use oos_core::change::Classes as C;

fn base(run: RunState) -> Ui {
    let mut ui = Ui::new(Source::Hw { shared: Default::default(), seen: 0 }, host::Host::headless());
    ui.m.run = run;
    ui.m.meas.clear();
    ui.m.meas2.clear();
    ui.step(0.04);
    change::reset(&mut ui);
    ui
}

#[test]
fn normalize_effective_classes_own_statistics_resets() {
    for run in [RunState::Run, RunState::Stop] {
        for repair in 0..3 {
            let mut ui = base(run);
            match repair {
                0 => ui.m.ch[0].offset = 1e6,
                1 => ui.m.zscale = ui.m.tscale + 1,
                _ => {
                    ui.m.ch[1].on = true;
                    ui.m.ch[0].on = false;
                    ui.m.spec_src = WaveId::Ch(0);
                    ui.m.eye_src = WaveId::Ch(0);
                }
            }
            ui.mstat = vec![MStat { n: 7, ..Default::default() }];
            change::begin_step(&mut ui, 0.04).unwrap();
            assert_eq!(ui.mstat.is_empty(), repair != 1, "only STATS repairs clear: {run:?}, repair {repair}");
            ui.mstat = vec![MStat { n: 9, ..Default::default() }];
            change::begin_step(&mut ui, 0.04).unwrap();
            assert_eq!(ui.mstat[0].n, 9, "idempotent normalize must not notify twice");
        }
    }
}

#[test]
fn default_setup_deliberately_invalidates_previous_statistics() {
    let mut ui = base(RunState::Stop);
    for _ in 0..2 {
        ui.mstat = vec![MStat { n: 7, ..Default::default() }];
        ui.key(key::DEFAULT);
        assert_eq!(ui.mstat[0].n, 7, "input defers the reset to acceptance");
        change::begin_step(&mut ui, 0.04).unwrap();
        assert!(ui.host.runtime().change.last.as_ref().unwrap().classes().contains(C::STATS));
        assert_eq!(change::counters(&ui).4, 0);
    }
}

#[test]
fn cursor_mode_preserves_temporal_views() {
    let mut ui = base(RunState::Stop);
    let mut trend = panes::Trend::default();
    trend.push(0.0, 1.0);
    trend.push(100.0, 2.0);
    ui.trend.push(trend);
    let mut eye = oos_dsp::eye::Eye::new(4, 4, 2, -1.0, 1.0);
    eye.records = 7;
    eye.hits[3] = 19;
    ui.azr.eye = Some(eye);
    ui.m.cursor = CursorMode::Time;
    change::begin_step(&mut ui, 0.04).unwrap();
    assert_eq!(ui.host.runtime().change.last.as_ref().unwrap().classes(), C::MAPPING);
    assert_eq!(ui.trend.len(), 1);
    assert_eq!(ui.trend[0].pts[0].v, 1.0);
    assert_eq!(ui.azr.eye.as_ref().unwrap().records, 7);
    assert_eq!(ui.azr.eye.as_ref().unwrap().hits[3], 19);
    assert_eq!(change::counters(&ui), ([0; 10], 0, 0, 0, 0));
}

#[test]
fn mask_toggle_only_advances_generation_for_an_effective_worker_route_change() {
    for already_slow in [false, true] {
        let mut ui = base(RunState::Run);
        if already_slow {
            ui.m.zoom = true;
            ui.step(0.04);
            change::reset(&mut ui);
        }
        let Source::Hw { shared, .. } = &ui.src else { unreachable!() };
        let shared = shared.clone();
        let mut frame = (*oos_hal::sync::lock(&shared).frame).clone();
        frame.seq = 1;
        frame.dt = 1e-6;
        frame.pre = 32;
        let id = change::acquisition::identity(&ui);
        frame.stamp = id.epoch();
        frame.cfg = id.gen();
        frame.ch[0] = (0..64).map(|i| (i as f32 - 32.0) * 0.01).collect();
        oos_hal::sync::lock(&shared).queue.push_back(frame.into());
        let stamp = ui.m.hw_stamp;
        let before = ui.fast_ok();
        ui.m.mask_on = true;
        crate::ui::change::edited(&mut ui);
        ui.step(0.04);
        let after = ui.fast_ok();
        let c = ui.host.runtime().change.last.as_ref().unwrap();
        assert_eq!(before != after, !already_slow);
        assert_eq!(c.classes().contains(C::ACQ), before != after);
        assert_eq!(c.identity().gen(), u64::from(before != after));
        assert!(c.classes().contains(C::STATS | C::MASK));
        assert_eq!(ui.m.hw_stamp, stamp);
        assert!(oos_hal::sync::lock(&shared).queue.is_empty());
        assert_eq!(ui.wfm_total, u64::from(already_slow), "D6 rejects old work only at an effective route boundary");
        assert_eq!(ui.last[0].is_some(), already_slow);
    }
}

#[test]
fn acquisition_mode_observes_the_missing_legacy_average_reset_as_d7() {
    for run in [RunState::Stop, RunState::Run] {
        let mut ui = base(run);
        let clear = ui.m.hw_avg_clear;
        ui.adjust(Item::AcqMode, 1);
        change::begin_step(&mut ui, 0.04).unwrap();
        assert_eq!(
            ui.host.runtime().change.last.as_ref().unwrap().classes(),
            C::ACQ | C::PHOSPHOR | C::STATS | C::HW_AVERAGE
        );
        assert_eq!(ui.m.hw_avg_clear, clear + 1, "HW_AVERAGE acts once at acceptance");
        assert_eq!(change::counters(&ui), ([0; 10], 0, 0, 0, 0), "{run:?}");
    }
}

#[test]
fn math_operation_observes_stateless_recomputation_as_d8() {
    let mut ui = base(RunState::Stop);
    ui.last[0] = Some(Record {
        t0: 0.0,
        dt: 1e-6,
        ch: [vec![-2.0, 3.0], vec![1.0, 2.0], vec![], vec![]],
        triggered: true,
        partial: false,
        raw: None,
        la: None,
        pairs: false,
    });
    ui.m.math = MathOp::Add;
    crate::ui::change::edited(&mut ui);
    change::begin_step(&mut ui, 0.04).unwrap();
    assert_eq!(ui.math_wave(), vec![-1.0, 5.0]);
    ui.m.math = MathOp::Sub;
    crate::ui::change::edited(&mut ui);
    change::begin_step(&mut ui, 0.04).unwrap();
    assert_eq!(ui.math_wave(), vec![-3.0, 1.0]);
    assert_eq!(ui.last[0].as_ref().unwrap().ch[0], vec![-2.0, 3.0]);
    assert_eq!(ui.host.runtime().change.last.as_ref().unwrap().classes(), C::MATH | C::STATS);
    assert_eq!(change::counters(&ui), ([0; 10], 0, 0, 0, 0));
}

#[test]
fn analyzer_edits_observe_the_actual_held_reset() {
    let mut ui = base(RunState::Stop);
    ui.m.ch[1].on = true;
    ui.step(0.04);
    change::reset(&mut ui);
    ui.mstat = vec![MStat { n: 7, ..Default::default() }];
    ui.azr.spec_avg = Some(Default::default());
    let mut waterfall = oos_dsp::spectrum::Waterfall::new(4);
    waterfall.rows.push_back(vec![1.0, 2.0]);
    ui.azr.wf = Some(waterfall);
    ui.adjust(Item::EyeSrc, 1);
    assert!(ui.azr.spec_avg.is_some(), "input defers the held reset");
    assert_eq!(ui.mstat[0].n, 7);
    change::begin_step(&mut ui, 0.04).unwrap();
    assert!(ui.azr.spec_avg.is_none());
    assert!(ui.azr.wf.as_ref().unwrap().rows.is_empty());
    assert_eq!(ui.host.runtime().change.last.as_ref().unwrap().classes(), C::VIEW);
    assert_eq!(change::counters(&ui), ([0; 10], 0, 0, 0, 0));
    ui.adjust(Item::EyeSrc, 0);
    change::begin_step(&mut ui, 0.04).unwrap();
    assert_eq!(change::counters(&ui), ([0; 10], 0, 0, 0, 0));
}

#[test]
fn hardware_mask_keeps_the_worker_route_but_can_change_the_read_format() {
    for tdiv in [200e-9, 10e-6] {
        let mut model = Model {
            dots: DotsMode::Off,
            tscale: tscales().iter().position(|&t| (t as f64 - tdiv).abs() < 1e-10).expect("listed time scale"),
            ..Model::default()
        };
        let config = crate::model::config();
        oos_core::change::normalize(&mut model, &config);
        let switches = crate::sysconf::Switches { compress: true, ..Default::default() };
        let before = board().want(&model, &switches).compress;
        assert_eq!(before > 1, tdiv > 1e-6);
        let needs = oos_core::change::UiAcq::of(false, true, false, false, Default::default());
        let mut shadow = oos_core::change::Shadow::new(&model, needs.with_read_format(before), &config).unwrap();
        model.mask_on = true;
        let after = board().want(&model, &switches).compress;
        assert_eq!(after, 1);
        let c = shadow.observe(&model, needs.with_read_format(after), &[], &config).unwrap();
        assert_eq!(c.classes(), C::STATS | C::MASK | if before != after { C::ACQ } else { C::NONE });
        assert_eq!(c.identity().gen(), u64::from(before != after));
    }
}

#[test]
fn zoom_knob_clamps_once_per_step_and_readback_follows_main_with_zoom_off() {
    let mut ui = base(RunState::Stop);
    ui.set_zoom(true);
    ui.m.zscale = ui.m.tscale;
    ui.adjust(Item::ZScale, 1);
    ui.adjust(Item::ZScale, -1);
    assert_eq!(ui.m.zscale, ui.m.tscale, "both input events precede the single clamp");
    ui.adjust(Item::ZScale, 1);
    assert_eq!(ui.m.zscale, ui.m.tscale + 1);
    change::begin_step(&mut ui, 0.04).unwrap();
    assert_eq!(ui.m.zscale, ui.m.tscale);
    assert_eq!(ui.value(Item::ZScale), si_short(tscales()[ui.m.tscale] as f64, "s"));
    ui.set_zoom(false);
    ui.m.delay = 0.001;
    change::begin_step(&mut ui, 0.04).unwrap();
    assert!(!ui.m.zoom);
    assert_eq!(ui.m.zdelay, ui.m.delay);
    assert_eq!(ui.value(Item::ZDelay), si(ui.m.delay as f64, "s"));
}

#[test]
fn mask_source_advances_generation_once_and_requests_statistics_resets_as_d9() {
    for run in [RunState::Run, RunState::Stop] {
        for hardware in [false, true] {
            let mut ui = base(run);
            if !hardware {
                ui.src = Source::Demo { t: 0.0 };
                change::reset(&mut ui);
            }
            let stamp = ui.m.hw_stamp;
            ui.mstat = vec![MStat { n: 7, ..Default::default() }];
            ui.m.mask_src = 1;
            crate::ui::change::edited(&mut ui);
            change::begin_step(&mut ui, 0.04).unwrap();
            let c = ui.host.runtime().change.last.as_ref().unwrap();
            assert_eq!(c.classes(), C::ACQ | C::STATS | C::MASK, "mask counts require a reset");
            let id = c.identity();
            assert_eq!((id.gen(), id.epoch(), id.stats_gen()), (1, 0, 1));
            assert!(ui.mstat.is_empty());
            assert_eq!(ui.m.hw_stamp, stamp, "the acting legacy generation still does not advance");
            assert_eq!(change::counters(&ui), ([0; 10], 0, 0, 0, 0), "{run:?}, hardware={hardware}");
            for _ in 0..2 {
                change::begin_step(&mut ui, 0.04).unwrap();
                let c = ui.host.runtime().change.last.as_ref().unwrap();
                assert_eq!(c.classes(), C::NONE, "same source must not reset mask counts again");
                assert_eq!(c.identity(), id);
                assert_eq!(change::counters(&ui), ([0; 10], 0, 0, 0, 0));
            }
        }
    }
}

#[test]
fn analyzer_settings_never_classify_or_clear_measurement_statistics() {
    for run in [RunState::Run, RunState::Stop] {
        for (item, expected) in [
            (Item::SpecSrc, C::MATH | C::VIEW),
            (Item::SpecCenter, C::MATH | C::VIEW),
            (Item::SpecSpan, C::MATH | C::VIEW),
            (Item::SpecRbw, C::MATH | C::VIEW),
            (Item::SpecWin, C::MATH | C::VIEW),
            (Item::SpecTrace, C::MATH | C::VIEW),
            (Item::SpecRef, C::MATH | C::VIEW),
            (Item::SpecWf, C::MATH | C::VIEW),
            (Item::SpecDist, C::MATH | C::VIEW),
            (Item::SpecMargin, C::MATH | C::VIEW),
            (Item::EyeSrc, C::VIEW),
            (Item::EyeRate, C::VIEW),
            (Item::PowV, C::VIEW),
            (Item::PowI, C::VIEW),
            (Item::PowMode, C::VIEW),
        ] {
            let mut ui = base(run);
            ui.m.ch[1].on = true;
            change::reset(&mut ui);
            ui.mstat = vec![MStat { n: 7, ..Default::default() }];
            ui.adjust(item, 1);
            change::begin_step(&mut ui, 0.04).unwrap();
            assert_eq!(ui.host.runtime().change.last.as_ref().unwrap().classes(), expected, "{run:?} {item:?}");
            assert_eq!(ui.mstat[0].n, 7, "{run:?} {item:?}");
            assert_eq!(change::counters(&ui).4, 0, "{run:?} {item:?}");
        }
    }
}
