// SPDX-License-Identifier: GPL-2.0-or-later

use super::super::*;
use crate::board::{Frame, HwShared};
use oos_hal::sync::lock;
use std::sync::Arc;

fn start(logic: bool) -> (Ui, HwShared) {
    let shared = HwShared::default();
    let mut ui = Ui::for_tests(Source::Hw { shared: shared.clone(), seen: 0 });
    ui.m.run = RunState::Run;
    ui.m.meas.clear();
    ui.m.meas2.clear();
    ui.m.tscale = tscales().iter().position(|&s| (s - 100e-6).abs() < 1e-10).unwrap();
    ui.m.mdepth = mdepths().iter().position(|d| d.1 == 10_000_000).unwrap();
    ui.m.la.on = logic;
    ui.m.la.show = u16::from(logic);
    ui.step(0.02);
    (ui, shared)
}

fn captured(ui: &Ui, seq: u64, logic: Option<la::LaRec>) -> Arc<Frame> {
    let id = change::acquisition::identity(ui);
    Arc::new(Frame {
        seq,
        cfg: id.gen(),
        stamp: id.epoch(),
        acq: (1e9, 10_000_000),
        dt: ui.m.time_per_div() as f64 * HDIV as f64 / 2048.0,
        pre: 2048,
        ch: [vec![if seq == 1 { -0.25 } else { 0.5 }; 4096], Vec::new(), Vec::new(), Vec::new()],
        fixed_ti: Some(2048.0),
        la: logic,
        ..Frame::default()
    })
}

#[test]
fn stop_drains_the_completed_serial_record_after_an_already_raw_record() {
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let (mut ui, shared) = start(false);
    let before = captured(&ui, 1, None);
    lock(&shared).queue.push_back(before.clone());
    ui.step(0.02);
    assert!(Arc::ptr_eq(&ui.last[0].as_ref().unwrap().raw.as_ref().unwrap().frame, &before));

    ui.m.run = RunState::Stop;
    ui.step(0.02);
    assert!(ui.stop_final.is_some() && lock(&shared).hold);
    assert!(lock(&shared).stop_view.is_none(), "wait for final capture before asking for the view");

    let complete = captured(&ui, 2, None);
    {
        let mut g = lock(&shared);
        g.queue.push_back(complete.clone());
        g.final_ack = g.final_req;
    }
    ui.step(0.02);
    ui.step(0.02);
    assert!(ui.stop_final.is_none() && lock(&shared).queue.is_empty());
    let shown = ui.last[0].as_ref().unwrap();
    assert!(Arc::ptr_eq(&shown.raw.as_ref().unwrap().frame, &complete));
    assert_eq!(shown.raw.as_ref().unwrap().frame.ch[0].len(), 4096);
    assert_eq!(ui.web_wave_bin().unwrap(), crate::store::wave_bin(&crate::wave::hw_record(&ui.m, complete), &ui.m));
}

#[test]
fn stop_freezes_the_completed_logic_capture_and_rereads_its_view() {
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let (mut ui, shared) = start(true);
    let logic = |word| la::LaRec { words: Arc::new(vec![0, word, word, 0]), t0: -3e-6, dt: 2e-6 };
    lock(&shared).queue.push_back(captured(&ui, 1, Some(logic(0))));
    ui.step(0.02);
    let old_csv = ui.web_la_csv().unwrap();

    ui.m.run = RunState::Stop;
    ui.step(0.02);
    assert!(ui.stop_final.is_some() && lock(&shared).stop_view.is_none());
    let final_frame = captured(&ui, 2, Some(logic(1)));
    {
        let mut g = lock(&shared);
        g.queue.push_back(final_frame.clone());
        g.final_ack = g.final_req;
    }
    ui.step(0.02);
    ui.step(0.02);
    ui.step(0.02);
    let saved = ui.web_la_csv().unwrap();
    assert_ne!(saved, old_csv);
    assert!(lock(&shared).stop_view.is_some(), "LA can request a stopped view");

    let reread = Arc::unwrap_or_clone(final_frame);
    lock(&shared).inject_stop_reply(reread.clone());
    ui.step(0.02);
    let shown = ui.last[0].as_ref().unwrap();
    assert!(Arc::ptr_eq(&shown.la.as_ref().unwrap().words, &reread.la.as_ref().unwrap().words));
    assert_eq!(ui.web_la_csv().unwrap(), saved);
    assert_eq!(shown.raw.as_ref().unwrap().frame.seq, 2);
    assert!(lock(&shared).stop_replies.frame.is_none());
}

#[test]
fn overlapping_normal_stop_on_raw_deep_or_logic_settles_in_one_step() {
    for logic in [false, true] {
        let _scope = crate::drawpipe::test_scope::Scope::new();
        let (mut ui, shared) = start(logic);
        ui.m.trig_mode = TrigMode::Normal;
        ui.step(0.02);
        let la = logic.then(|| la::LaRec { words: Arc::new(vec![1; 4]), t0: -3e-6, dt: 2e-6 });
        let previous = captured(&ui, 1, la);
        lock(&shared).queue.push_back(previous.clone());
        ui.step(0.02);

        ui.m.run = RunState::Stop;
        ui.step(0.02);
        let g = lock(&shared);
        assert!(g.hold && g.stop_view.is_some());
        assert_eq!((ui.stop_final, g.final_req), (None, 0));
        assert_eq!(ui.m.status, TrigStatus::Stop);
        assert!(Arc::ptr_eq(&ui.last[0].as_ref().unwrap().raw.as_ref().unwrap().frame, &previous));
    }
}

#[test]
fn stop_drops_replies_for_the_previous_geometry_or_capture() {
    for _ in 0..2 {
        let _scope = crate::drawpipe::test_scope::Scope::new();
        let (mut ui, shared) = start(false);
        ui.m.trig_mode = TrigMode::Normal;
        ui.step(0.02);
        lock(&shared).queue.push_back(captured(&ui, 1, None));
        ui.step(0.02);
        ui.m.run = RunState::Stop;
        ui.step(0.02);
        let previous_geometry = lock(&shared).stop_view.unwrap();
        let late = Arc::unwrap_or_clone(captured(&ui, 10, None));

        ui.m.delay += ui.m.time_per_div();
        lock(&shared).stop_replies.frame = Some((previous_geometry, late.clone()));
        ui.step(0.02);
        let previous_capture = lock(&shared).stop_view.unwrap();
        assert_ne!(previous_geometry, previous_capture);
        assert_eq!(ui.last[0].as_ref().unwrap().raw.as_ref().unwrap().frame.seq, 1);

        ui.m.run = RunState::Run;
        ui.step(0.02);
        lock(&shared).queue.push_back(captured(&ui, 2, None));
        ui.step(0.02);
        ui.m.run = RunState::Stop;
        ui.step(0.02);
        assert_ne!(lock(&shared).stop_view, Some(previous_capture));
        lock(&shared).stop_replies.frame = Some((previous_capture, late.clone()));
        ui.step(0.02);
        assert_eq!(ui.last[0].as_ref().unwrap().raw.as_ref().unwrap().frame.seq, 2);

        lock(&shared).inject_stop_reply(late);
        ui.step(0.02);
        assert_eq!(ui.last[0].as_ref().unwrap().raw.as_ref().unwrap().frame.seq, 10);
    }
}

#[test]
fn serial_normal_stop_drains_a_shallow_final_record() {
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let (mut ui, shared) = start(false);
    ui.m.trig_mode = TrigMode::Normal;
    ui.step(0.02);
    let shallow = |ui: &Ui, seq| {
        let mut frame = Arc::unwrap_or_clone(captured(ui, seq, None));
        frame.acq.1 = 100_000;
        Arc::new(frame)
    };
    {
        let mut g = lock(&shared);
        g.record_flow.serial = true;
        g.queue.push_back(shallow(&ui, 1));
    }
    ui.step(0.02);
    ui.m.run = RunState::Stop;
    ui.step(0.02);
    assert!(ui.stop_final.is_some() && lock(&shared).stop_view.is_none());
    let complete = shallow(&ui, 2);
    {
        let mut g = lock(&shared);
        g.queue.push_back(complete.clone());
        g.final_ack = g.final_req;
    }
    ui.step(0.02);
    ui.step(0.02);
    assert!(ui.stop_final.is_none());
    assert!(Arc::ptr_eq(&ui.last[0].as_ref().unwrap().raw.as_ref().unwrap().frame, &complete));
}

#[test]
fn serial_stop_without_a_final_record_keeps_the_displayed_host_record() {
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let (mut ui, shared) = start(false);
    ui.m.trig_mode = TrigMode::Normal;
    ui.step(0.02);
    let previous = captured(&ui, 1, None);
    {
        let mut g = lock(&shared);
        g.record_flow.serial = true;
        g.queue.push_back(previous.clone());
    }
    ui.step(0.02);
    ui.m.run = RunState::Stop;
    ui.step(0.02);
    assert!(ui.stop_final.is_some());
    {
        let mut g = lock(&shared);
        g.final_ack = g.final_req;
    }
    ui.step(0.02);
    assert!(ui.stop_final.is_none());
    assert!(Arc::ptr_eq(&ui.last[0].as_ref().unwrap().raw.as_ref().unwrap().frame, &previous));
}
