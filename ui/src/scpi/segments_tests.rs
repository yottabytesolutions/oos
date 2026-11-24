// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

#[test]
fn segment_times_during_recording_answers_empty_state() {
    let shared = crate::board::HwShared::default();
    let mut ui = Ui::for_tests(crate::wave::Source::Hw { shared: shared.clone(), seen: 0 });
    oos_hal::sync::lock(&shared).seg.phase = crate::board::SegPhase::Recording;
    let mut scpi = Scpi::open(0, false);
    let canvas = Canvas::new(1, 1);
    scpi.exec_line(&mut ui, &canvas, ":BODE:GEN \"\"");
    assert_eq!(scpi.response(&mut ui, &canvas, ":ACQuire:SEGMents:TIMes?"), Some(b"\n".to_vec()));
    assert_eq!(scpi.response(&mut ui, &canvas, ":BODE:GEN?"), Some(b"\n".to_vec()));
    assert_eq!(scpi.response(&mut ui, &canvas, "*OPC?"), Some(b"1\n".to_vec()));
    assert_eq!(scpi.response(&mut ui, &canvas, ":ACQ:SEGM:TIM?;*OPC?"), Some(b";1\n".to_vec()));
    assert_eq!(scpi.response(&mut ui, &canvas, "*CLS"), None);
}

fn record_session() -> (Scpi, Ui, Canvas, crate::board::HwShared) {
    let shared = crate::board::HwShared::default();
    let mut ui = Ui::for_tests(crate::wave::Source::Hw { shared: shared.clone(), seen: 0 });
    (ui.m.run, ui.m.zoom, ui.m.trig_mode) = (RunState::Stop, false, crate::model::TrigMode::Normal);
    ui.m.tscale = crate::model::tscales().iter().position(|&v| v == 1e-6).unwrap();
    ui.m.ch.iter_mut().enumerate().for_each(|(index, ch)| ch.on = index < 2);
    ui.m.trig_src = 1;
    ui.step(0.02);
    (Scpi::disconnected(), ui, Canvas::new(1, 1), shared)
}

fn counts() -> (u64, u64) {
    use std::sync::atomic::Ordering::Relaxed;
    let counters = crate::drawpipe::counters();
    (counters.acquired.load(Relaxed), counters.shown.load(Relaxed))
}

#[test]
fn record_route_five_singles_read_and_show_five() {
    use crate::board::scripted::RecordWorker;
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let (mut scpi, mut ui, cv, shared) = record_session();
    let mut worker = RecordWorker::default();
    for n in 1..=5 {
        let mut program = Program::new(":SING;*OPC?");
        assert!(scpi.poll_program(&mut ui, &cv, &mut program).is_none());
        ui.step(0.02);
        assert_eq!(worker.tick(&shared, ui.switches()), 1);
        assert_eq!(worker.tick(&shared, ui.switches()), 0);
        ui.step(0.02);
        assert_eq!(scpi.poll_program(&mut ui, &cv, &mut program), Some(b"1".to_vec()));
        assert_eq!(counts(), (n, n));
    }
}

fn exercise_software_segments(single: bool) {
    use crate::board::scripted::RecordWorker;
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let (mut scpi, mut ui, cv, shared) = record_session();
    scpi.exec_line(&mut ui, &cv, ":ACQ:SEGM 100;:ACQ:SEGM:HARD OFF");
    let mut program = Program::new(if single { ":SING;*OPC?" } else { ":RUN" });
    assert_eq!(scpi.poll_program(&mut ui, &cv, &mut program), (!single).then(Vec::new));
    let mut worker = RecordWorker::default();
    let mut completed = false;
    for _ in 0..=100 {
        ui.step(0.02);
        let left = 100 - counts().1;
        if left == 0 {
            completed = true;
            assert_eq!(worker.tick(&shared, ui.switches()), 0, "no final overread before STOP");
            break;
        }
        let read = worker.tick(&shared, ui.switches());
        assert!(read > 0 && u64::from(read) <= left, "read {read} with {left} segments left");
        assert_eq!(worker.tick(&shared, ui.switches()), 0, "no successor before the UI drains the batch");
        if single {
            assert!(scpi.poll_program(&mut ui, &cv, &mut program).is_none());
        }
    }
    assert!(completed, "fixture runs at most one batch per segment");
    ui.step(0.02);
    assert_eq!(ui.m.run, RunState::Stop);
    assert_eq!(counts(), (100, 100));
    if single {
        assert_eq!(scpi.poll_program(&mut ui, &cv, &mut program), Some(b"1".to_vec()));
    }
}

#[test]
fn record_route_software_segments_run() {
    exercise_software_segments(false);
}

#[test]
fn record_route_software_segments_single() {
    exercise_software_segments(true);
}
