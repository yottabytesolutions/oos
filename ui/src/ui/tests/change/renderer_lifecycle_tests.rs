// SPDX-License-Identifier: GPL-2.0-or-later
//! Actual accepted acquisition/view publication, including workflow changes which reuse worker maps.
use super::*;
use crate::drawpipe::budget::WORK_BUDGET;
#[test]
fn admission_starts_cold_after_actual_stop_single_run_and_clear() {
    assert!(check(3) > check(15), "the startup model accounts for enabled channels");
}
fn check(mask: u32) -> u32 {
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let shared: crate::board::HwShared = Default::default();
    let mut ui = Ui::for_tests(Source::Hw { shared: shared.clone(), seen: 0 });
    configure_normal_channels(&mut ui, mask);
    ui.m.run = RunState::Run;
    change::acquisition::script_prepare_route(&mut ui, false);
    ui.step(0.04);
    let pipe = crate::drawpipe::pipe().unwrap();
    let old = change::acquisition::identity(&ui);

    let initial = pipe.grant(old, 142, WORK_BUDGET);
    assert!(!initial.serial());
    let mut scpi = crate::scpi::Scpi::disconnected();
    let cv = Canvas::new(970, 448);
    scpi.exec_line(&mut ui, &cv, ":STOP");
    ui.step(0.04);
    {
        let mut source = shared.lock().unwrap();
        source.final_ack = source.final_req;
    }
    ui.step(0.04);
    change::acquisition::script_prepare_route(&mut ui, false);
    assert_eq!(ui.m.run, RunState::Stop);
    assert!(!pipe.grant(change::acquisition::identity(&ui), 142, WORK_BUDGET).serial());
    scpi.exec_line(&mut ui, &cv, ":SING");
    ui.step(0.04);
    change::acquisition::script_prepare_route(&mut ui, false);
    assert!(!pipe.grant(change::acquisition::identity(&ui), 142, WORK_BUDGET).serial());
    scpi.exec_line(&mut ui, &cv, ":RUN");
    ui.step(0.04);
    change::acquisition::script_prepare_route(&mut ui, false);
    let current = change::acquisition::identity(&ui);
    assert_eq!(pipe.grant(current, 142, WORK_BUDGET), initial);
    scpi.exec_line(&mut ui, &cv, ":CLEAR");
    ui.step(0.04);
    change::acquisition::script_prepare_route(&mut ui, false);
    let cleared = change::acquisition::identity(&ui);
    assert_ne!(cleared, current);
    assert_eq!(pipe.grant(cleared, 142, WORK_BUDGET), initial);
    initial.records()
}

fn configure_normal_channels(ui: &mut Ui, mask: u32) {
    for (key, value) in [
        ("tdiv", "0.000001"),
        ("trig_mode", "Normal"),
        ("trig_type", "Edge"),
        ("acq", "Normal"),
        ("mdepth", "0"),
        ("cores", "true"),
        ("dots", "Auto"),
        ("persist", "0"),
        ("meas", ""),
        ("meas2", ""),
        ("ch1.on", "true"),
        ("ch2.on", "true"),
        ("ch3.on", "true"),
        ("ch4.on", "true"),
    ] {
        crate::store::set(&mut ui.m, key, value).unwrap();
    }
    for c in 0..4 {
        ui.m.ch[c].on = mask & (1 << c) != 0;
    }
}
