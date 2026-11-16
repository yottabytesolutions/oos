// SPDX-License-Identifier: GPL-2.0-or-later

use super::super::*;
use change::{acquisition, counters, reset, timing};

#[test]
fn settings_marked_has_exact_reviewed_oracle_deltas() {
    let mut ui = Ui::new(Source::Demo { t: 0.0 }, host::Host::headless());
    ui.step(0.04);
    reset(&mut ui);
    ui.settings_marked();
    ui.step(0.04);
    assert_eq!(counters(&ui), ([0, 1, 0, 1, 0, 0, 0, 0, 0, 0], 0, 0, 0, 0));
}

#[test]
fn picture_consumer_has_its_own_timing_samples_and_state_counter() {
    let mut ui = Ui::new(Source::Hw { shared: Default::default(), seen: 0 }, host::Host::headless());
    ui.m.meas.clear();
    ui.m.meas2.clear();
    ui.step(0.04);
    reset(&mut ui);
    for i in 1..=120 {
        if i % 2 == 0 {
            ui.m.delay = (i / 2 % 2) as f32 * ui.m.time_per_div();
            ui.settings_marked();
        }
        ui.step(0.04);
    }
    assert!(timing(&ui).picture_p99().unwrap() > 0);
    assert!(timing(&ui).picture_max().unwrap() >= timing(&ui).picture_p99().unwrap());
    assert_eq!(counters(&ui), ([0; 10], 0, 0, 0, 0));
    let text = timing(&ui).text();
    assert!(text.contains("stage picture n=60 p99_ns="), "{text}");
    let cv = Canvas::new(ui.scr.w, ui.scr.h);
    let state = String::from_utf8(crate::scpi::Scpi::open(0u16, false).exec_line(&mut ui, &cv, ":SYST:STAT?")).unwrap();
    assert!(state.contains("max_ns="), "{state}");
    assert!(state.contains("stage picture n=60 p99_ns="), "{state}");
}
#[test]
fn script_route_updates_an_existing_prepared_worker_policy() {
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let mut ui = crate::ui::Ui::new(
        crate::wave::Source::Hw { shared: Default::default(), seen: 0 },
        crate::ui::host::Host::headless(),
    );
    ui.m.meas.clear();
    ui.m.meas2.clear();
    ui.m.draw_batch = true;
    let id = crate::ui::change::acquisition::script_prepare_route(&mut ui, false);
    let pipe = crate::drawpipe::pipe(crate::drawpipe::Access::Existing).unwrap();
    assert!(pipe.accepting());
    assert_eq!(crate::ui::change::acquisition::script_prepare_route(&mut ui, true), id);
    assert!(!pipe.accepting(), "record-fed scripts must disable an already prepared worker view");
    assert_eq!(crate::ui::change::acquisition::script_prepare_route(&mut ui, false), id);
    assert!(pipe.accepting(), "worker-fed scripts can reuse the same accepted preparation");
}

#[test]
fn batch_worker_view_stays_enabled_during_stop_final_read() {
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let mut ui = crate::ui::Ui::new(
        crate::wave::Source::Hw { shared: Default::default(), seen: 0 },
        crate::ui::host::Host::headless(),
    );
    ui.m.meas.clear();
    ui.m.meas2.clear();
    ui.m.draw_batch = true;
    ui.step(0.04);
    let pipe = crate::drawpipe::pipe(crate::drawpipe::Access::Existing).unwrap();
    assert!(pipe.accepting());
    let frame = std::sync::Arc::new(crate::board::Frame {
        envelope: true,
        acq: (625e6, 100_000),
        dt: 1e-8,
        ch: [vec![-0.25, 0.25, -0.25, 0.25], Vec::new(), Vec::new(), Vec::new()],
        ..Default::default()
    });
    ui.last[0] = Some(crate::board::to_record(&frame, &ui.m, 64, 1e-3, 0.0));
    feed_pending_batch(&ui, pipe);
    ui.m.run = RunState::Stop;
    ui.step(0.04);
    assert!(ui.stop_final.is_some());
    assert!(pipe.accepting(), "STOP must keep merging the in-flight batch until its final read");
    assert_eq!(crate::drawpipe::counters().read(), (0, 26, 0));
    assert!(ui.ph.map[0].iter().any(|&v| v != 0), "the completed worker batch reaches the picture");
    let Source::Hw { shared, .. } = &ui.src else { unreachable!() };
    let id = acquisition::identity(&ui);
    let final_frame =
        crate::board::Frame { seq: 2, cfg: id.gen(), stamp: id.epoch(), envelope: false, ..(*frame).clone() };
    {
        let mut board = oos_hal::sync::lock(shared);
        board.queue.push_back(final_frame);
        board.final_ack = ui.final_req;
    }
    ui.step(0.04);
    ui.step(0.04);
    assert!(ui.stop_final.is_none());
    assert_eq!(ui.last[0].as_ref().unwrap().raw.as_ref().unwrap().frame.seq, 2);
    ui.step(0.04);
    assert!(!pipe.accepting(), "workers retire after final-read completion");
}
fn feed_pending_batch(ui: &Ui, pipe: &crate::drawpipe::Pipe) {
    use crate::board::scripted::{self, Batch, BatchKind, BatchSource};
    let mut source = scripted::source(BatchKind::Raw, 123);
    let batch = source.next_batch(&ui.m, ui.switches(), 27, None).unwrap();
    let (fb, decode) = (batch.frame_bytes(), batch.decode());
    pipe.feed_with(Box::new(batch.into_bytes()), fb, 0..26, acquisition::identity(ui), decode, None, Box::new(|| {}));
    crate::drawpipe::host::finish_batch(pipe, 1).unwrap();
}

#[test]
fn draw_timing_state_query_appends_completed_steps_and_independent_renders() {
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let mut ui = Ui::for_tests(Source::Hw { shared: Default::default(), seen: 0 });
    let mut cv = Canvas::new(ui.scr.w, ui.scr.h);
    ui.m.persist = 1;
    for _ in 0..50 {
        ui.step(0.04);
    }
    ui.render(&mut cv);
    let state = String::from_utf8(crate::scpi::Scpi::open(0u16, false).exec_line(&mut ui, &cv, ":SYST:STAT?")).unwrap();
    assert!(state.find("diag producer.clone_size").unwrap() < state.find("diag draw.decay").unwrap());
    for label in ["decay", "fast", "merge", "records", "grade"] {
        let block = state.split_once(&format!("diag draw.{label} ")).unwrap().1.split(" diag ").next().unwrap();
        assert!(block.starts_with("n=50 total=50 p50_ns="), "{block}");
        assert!(!block.contains("pending"), "{block}");
    }
    assert!(state.contains("draw.render n=1 total=1 p50_ns="));
    ui.m.run = RunState::Stop;
    ui.step(0.04);
    assert!(timing(&ui).text().contains("draw.decay n=51 total=51"));
    let frames = ui.frames;
    ui.start_up_begin(None);
    assert!(ui.start_up_active());
    ui.render(&mut cv);
    assert_eq!(ui.frames, frames, "start-up retains its existing early return");
    assert!(timing(&ui).text().contains("draw.render n=2 total=2"));
}

#[test]
fn state_query_real_reply_has_separate_panel_and_view_renders() {
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let mut ui = Ui::for_tests(Source::Demo { t: 0.0 });
    let mut cv = Canvas::new(ui.scr.w, ui.scr.h);
    ui.step(0.04);
    ui.render(&mut cv);
    ui.render_view(&mut cv);
    assert!(timing(&ui).text().contains("draw.render n=1 total=1"));
    assert!(timing(&ui).text().contains("draw.view_render n=1 total=1"));
    let frames = ui.frames;
    ui.start_up_begin(None);
    ui.render_view(&mut cv);
    assert_eq!(ui.frames, frames);
    assert!(timing(&ui).text().contains("draw.render n=1 total=1"));
    assert!(timing(&ui).text().contains("draw.view_render n=2 total=2"));
    let state =
        String::from_utf8(crate::scpi::Scpi::open(0u16, false).exec_line(&mut ui, &cv, ":SYSTem:STATe?")).unwrap();
    assert!(state.starts_with("fps "), "{state}");
    assert!(state.contains("diag draw.render n=1 total=1"), "{state}");
    assert!(state.contains("diag draw.view_render n=2 total=2"), "{state}");
    let fixture = include_str!("../../../../tools/tests/fixtures/perfmap/state.txt");
    let labels = |reply: &str| {
        reply
            .split(" diag ")
            .skip(1)
            .map(|block| block.split_whitespace().next().unwrap().to_owned())
            .collect::<Vec<_>>()
    };
    assert_eq!(labels(&state), labels(fixture), "fixture diagnostic labels must track the real handler");
    // Retain the actual production reply for fixture regeneration on restricted hosts.
    println!("{state}");
}
