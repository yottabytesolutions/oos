// SPDX-License-Identifier: GPL-2.0-or-later

use super::super::*;
use crate::board::HwShared;

fn hardware(mode: TrigMode) -> Ui {
    let mut ui = Ui::for_tests(Source::Hw { shared: HwShared::default(), seen: 0 });
    ui.m.trig_mode = mode;
    ui.step(0.04);
    ui
}

#[test]
fn run_after_single_restores_the_previous_sweep() {
    for mode in [TrigMode::Normal, TrigMode::Auto] {
        for completed in [false, true] {
            let mut ui = hardware(mode);
            ui.key(key::SINGLE);
            ui.step(0.04);
            assert_eq!(ui.m.trig_mode, TrigMode::Single);
            ui.key(key::SINGLE);
            ui.step(0.04);
            if completed {
                change::proposals::stop(&mut ui);
                ui.step(0.04);
            } else {
                ui.key(key::RUNSTOP);
                ui.step(0.04);
            }
            assert_eq!(ui.m.run, RunState::Stop);
            assert_eq!(ui.m.trig_mode, TrigMode::Single);
            ui.key(key::RUNSTOP);
            ui.step(0.04);
            assert_eq!((ui.m.run, ui.m.trig_mode), (RunState::Run, mode));
            assert_eq!(change::counters(&ui).4, 0);
        }
    }
}

#[test]
fn scpi_run_after_single_restores_the_previous_sweep() {
    for mode in [TrigMode::Normal, TrigMode::Auto] {
        for stop in [false, true] {
            let mut ui = hardware(mode);
            let mut remote = crate::scpi::Scpi::open(0u16, false);
            let canvas = Canvas::new(1, 1);
            remote.exec_line(&mut ui, &canvas, ":SING;:SING");
            assert_eq!((ui.m.run, ui.m.trig_mode), (RunState::Run, TrigMode::Single));
            if stop {
                remote.exec_line(&mut ui, &canvas, ":STOP");
                assert_eq!(ui.m.run, RunState::Stop);
            }
            remote.exec_line(&mut ui, &canvas, ":RUN");
            assert_eq!((ui.m.run, ui.m.trig_mode), (RunState::Run, mode));
            assert_eq!(change::counters(&ui).4, 0);
        }
    }
}

#[test]
fn mask_run_after_single_restores_the_previous_sweep() {
    for (mode, sweep) in [(TrigMode::Normal, "NORM"), (TrigMode::Auto, "AUTO")] {
        for stop in [false, true] {
            let mut ui = hardware(mode);
            ui.m.acq = AcqMode::Average;
            let mut remote = crate::scpi::Scpi::open(0u16, false);
            let canvas = Canvas::new(1, 1);
            remote.exec_line(&mut ui, &canvas, &format!(":MASK:ENAB ON;:TRIG:SWE {sweep};:SING"));
            if stop {
                remote.exec_line(&mut ui, &canvas, ":STOP");
            }
            assert_eq!(ui.m.trig_mode, TrigMode::Single);
            assert_eq!(ui.m.run, if stop { RunState::Stop } else { RunState::Run });
            let before = change::acquisition::identity(&ui);
            ui.mstat.resize(1, MStat::default());
            ui.mstat[0].n = 7;
            assert_eq!(remote.exec_line(&mut ui, &canvas, ":MASK:OPER RUN;:MASK:OPER?"), b"RUN");
            assert_eq!((ui.m.run, ui.m.trig_mode), (RunState::Run, mode));
            assert_eq!(change::counters(&ui).4, 0);
            let after = change::acquisition::identity(&ui);
            assert_eq!(after.gen(), before.gen());
            assert_eq!(after.stats_gen(), before.stats_gen());
            assert_eq!(after.epoch(), before.epoch() + u64::from(stop));
            assert_eq!(ui.mstat[0].n, 7);
        }
    }
}

#[test]
fn replay_run_after_single_restores_the_previous_sweep() {
    for mode in [TrigMode::Normal, TrigMode::Auto] {
        let mut ui = hardware(mode);
        replay_source(&mut ui);
        ui.key(key::SINGLE);
        ui.step(0.04);
        assert_eq!(ui.m.run, RunState::Stop);
        assert_eq!(ui.replay.captures, 1);
        ui.key(key::RUNSTOP);
        assert_eq!((ui.m.run, ui.m.trig_mode), (RunState::Run, mode));
    }
}

#[test]
fn live_run_after_single_restores_its_sweep_after_reference_or_sim_edits() {
    for (mode, selected) in [(TrigMode::Auto, "NORM"), (TrigMode::Normal, "AUTO")] {
        for sim in [false, true] {
            let mut ui = hardware(mode);
            ui.key(key::SINGLE);
            ui.step(0.04);
            ui.key(key::RUNSTOP);
            ui.step(0.04);
            assert_eq!((ui.m.run, ui.m.trig_mode), (RunState::Stop, TrigMode::Single));
            if sim {
                ui.sim_play_set(true).unwrap();
                ui.wait_sim_generation();
                ui.step(0.04);
            } else {
                replay_source(&mut ui);
            }
            assert!(ui.record_mode());
            let mut remote = crate::scpi::Scpi::open(0u16, false);
            remote.exec_line(&mut ui, &Canvas::new(1, 1), &format!(":TRIG:SWE {selected};:TRIG:SWE SING"));
            ui.return_to_live();
            ui.step(0.04);
            assert!(!ui.record_mode());
            assert_eq!((ui.m.run, ui.m.trig_mode), (RunState::Stop, TrigMode::Single));
            ui.key(key::RUNSTOP);
            ui.step(0.04);
            assert_eq!((ui.m.run, ui.m.trig_mode), (RunState::Run, mode), "SIM: {sim}");
        }
    }
}

fn replay_source(ui: &mut Ui) {
    ui.refs[0] = Some(RefWave {
        src: 0,
        color: 0,
        label: "R1".into(),
        t0: 0.0,
        dt: 1e-9,
        v: (0..5000).map(|i| if i % 40 < 20 { -1.0 } else { 1.0 }).collect::<Vec<_>>().into(),
        vpd: 0.5,
        offset: 0.0,
        on: true,
    });
    assert!(ui.set_trigger_source(WaveId::Ref(0), false));
    ui.m.trig_type = TrigType::Edge;
    ui.m.trig_level = 0.0;
}

#[test]
fn explicit_sweep_changes_replace_the_remembered_single_sweep() {
    for (initial, selected) in [(TrigMode::Normal, "AUTO"), (TrigMode::Auto, "NORM")] {
        let mut ui = hardware(initial);
        let mut remote = crate::scpi::Scpi::open(0u16, false);
        let canvas = Canvas::new(1, 1);
        let program = format!(":SING;:TRIG:SWE {selected};:TRIG:SWE SING;:RUN;:TRIG:SWE?");
        assert_eq!(remote.exec_line(&mut ui, &canvas, &program), selected.as_bytes());
    }
}

#[test]
fn setup_sweep_changes_replace_the_remembered_single_sweep() {
    for (initial, selected, reply) in [(TrigMode::Normal, "Auto", "AUTO"), (TrigMode::Auto, "Normal", "NORM")] {
        let mut ui = hardware(initial);
        let mut remote = crate::scpi::Scpi::open(0u16, false);
        let program = format!(":SING;:SYST:SET trig_mode = {selected};:SYST:SET trig_mode = Single;:RUN;:TRIG:SWE?");
        assert_eq!(remote.exec_line(&mut ui, &Canvas::new(1, 1), &program), reply.as_bytes());
    }
}

#[test]
fn scpi_run_while_single_is_armed_preserves_record_statistics() {
    for mode in [TrigMode::Normal, TrigMode::Auto] {
        for replay in [false, true] {
            let mut ui = hardware(mode);
            ui.m.acq = AcqMode::Average;
            if replay {
                replay_source(&mut ui);
            }
            let mut remote = crate::scpi::Scpi::open(0u16, false);
            let canvas = Canvas::new(1, 1);
            remote.exec_line(&mut ui, &canvas, ":SING");
            assert_eq!((ui.m.run, ui.m.trig_mode), (RunState::Run, TrigMode::Single));
            let before = change::acquisition::identity(&ui);
            ui.mstat.resize(1, MStat::default());
            ui.mstat[0].n = 7;
            remote.exec_line(&mut ui, &canvas, ":RUN");
            assert_eq!((ui.m.run, ui.m.trig_mode), (RunState::Run, mode));
            assert_eq!(change::acquisition::identity(&ui), before, "{mode:?}, replay: {replay}");
            assert_eq!(ui.mstat[0].n, 7);
        }
    }
}

#[test]
fn replay_scpi_run_after_single_restores_the_previous_sweep() {
    for mode in [TrigMode::Normal, TrigMode::Auto] {
        for completed in [false, true] {
            let mut ui = hardware(mode);
            replay_source(&mut ui);
            let mut remote = crate::scpi::Scpi::open(0u16, false);
            let canvas = Canvas::new(1, 1);
            remote.exec_line(&mut ui, &canvas, ":SING");
            if completed {
                ui.step(0.04);
                assert_eq!(ui.replay.captures, 1);
            } else {
                remote.exec_line(&mut ui, &canvas, ":STOP");
            }
            assert_eq!(ui.m.run, RunState::Stop);
            remote.exec_line(&mut ui, &canvas, ":RUN");
            assert_eq!((ui.m.run, ui.m.trig_mode), (RunState::Run, mode));
        }
    }
}

#[test]
fn explicit_sweep_after_run_resets_acquisition_and_statistics() {
    for (mode, selected) in [(TrigMode::Auto, "NORM"), (TrigMode::Normal, "AUTO")] {
        let mut ui = hardware(mode);
        ui.key(key::SINGLE);
        ui.step(0.04);
        change::proposals::stop(&mut ui);
        let before = change::acquisition::identity(&ui);
        let mut remote = crate::scpi::Scpi::open(0u16, false);
        change::intake(&mut ui, |ui| {
            remote.exec_line(ui, &Canvas::new(1, 1), &format!(":RUN;:TRIG:SWE {selected}"));
        });
        let after = change::acquisition::identity(&ui);
        assert_eq!(after.gen(), before.gen() + 1);
        assert_eq!(after.stats_gen(), before.stats_gen() + 1);
        assert_eq!(after.epoch(), before.epoch() + 1);
        assert_eq!(change::counters(&ui).4, 0);
    }
}

#[test]
fn leaving_a_reference_session_does_not_restore_a_stale_single_arm() {
    let mut ui = hardware(TrigMode::Normal);
    ui.key(key::SINGLE);
    ui.step(0.04);
    assert!(ui.trigger.single_armed);
    replay_source(&mut ui);
    ui.step(0.04);
    assert_eq!(ui.m.run, RunState::Stop);
    ui.step(0.04);
    assert!(!ui.trigger.single_armed);
    ui.return_to_live();
    assert!(!ui.trigger.single_armed);
    ui.step(0.04);
    assert_eq!((ui.m.run, ui.m.trig_mode), (RunState::Run, TrigMode::Single));
    assert!(ui.trigger.single_armed);
}

#[test]
fn invalid_setup_sweep_preserves_single_and_its_remembered_mode() {
    for value in ["auto", "NORM", "invalid", ""] {
        let mut ui = hardware(TrigMode::Normal);
        let mut remote = crate::scpi::Scpi::open(0u16, false);
        let canvas = Canvas::new(1, 1);
        remote.exec_line(&mut ui, &canvas, &format!(":SING;:SYST:SET trig_mode = {value}"));
        assert_eq!(ui.m.trig_mode, TrigMode::Single);
        let error = remote.exec_line(&mut ui, &canvas, ":SYST:ERR?");
        assert!(String::from_utf8(error).unwrap().starts_with("-224,"));
        assert_eq!(remote.exec_line(&mut ui, &canvas, ":RUN;:TRIG:SWE?"), b"NORM");
    }
}
