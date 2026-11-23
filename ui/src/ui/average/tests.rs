// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::board::Frame;
use crate::drawpipe::{self, Counters, Pipe, RawBatch};
use std::{any::Any, sync::Arc};

struct Codes(Vec<u8>);

impl RawBatch for Codes {
    fn bytes(&self) -> &[u8] {
        &self.0
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any + Send> {
        self
    }
}

fn ui() -> Ui {
    let mut ui = Ui::for_tests(Source::Hw { shared: Default::default(), seen: 0 });
    ui.m.acq = AcqMode::Average;
    ui.m.avg_hw = false;
    ui.m.avg_count = 2;
    ui.m.tscale = tscales().iter().position(|&t| (t - 100e-9).abs() < 1e-12).unwrap();
    ui.m.meas.clear();
    ui.m.meas2.clear();
    ui.step(0.04);
    assert!(ui.fast_ok());
    ui
}

fn submit(ui: &Ui, first: usize, codes: &[u8]) {
    let id = change::acquisition::identity(ui);
    let decode = |value, f: &mut Frame| {
        f.dt = 2e-9;
        f.pre = 280;
        f.ch[0].clear();
        f.ch[0].resize(560, f32::from(value) / 16.0);
    };
    let mut newest =
        Frame { seq: (first + codes.len()) as u64, cfg: id.gen(), stamp: id.epoch(), ..Default::default() };
    decode(*codes.last().unwrap(), &mut newest);
    let pipe = drawpipe::pipe().unwrap();
    Counters::add(&pipe.batch_counters().acquired, codes.len() as u64);
    assert!(pipe
        .feed_prepared(
            Box::new(Codes(codes.to_vec())),
            1,
            0..codes.len(),
            (id, std::time::Instant::now()),
            Arc::new(move |_, bytes, f| decode(bytes[0], f)),
            Some(Arc::new(newest)),
        )
        .is_none());
    let _ = drawpipe::host::finish_batch(pipe, first as u64).unwrap();
}

fn advance(ui: &mut Ui, first: usize, codes: &[u8]) -> f32 {
    submit(ui, first, codes);
    ui.step(0.04);
    ui.last[0].as_ref().unwrap().ch[0][1800]
}

#[test]
fn batch_average_counts_every_fold_and_retains_the_newest_raw_for_readers() {
    let _scope = drawpipe::test_scope::Scope::new();
    let mut ui = ui();
    let value = advance(&mut ui, 0, &[2, 4, 6, 8]);
    assert!((value - 0.390625).abs() < 1e-6);
    let rec = ui.last[0].as_ref().unwrap();
    let raw = rec.raw.as_ref().unwrap();
    assert_eq!((raw.frame.seq, raw.frame.ch[0][280]), (4, 0.5));
    assert_eq!(ui.history.len(), 1);
    assert_eq!(ui.wfm_total, 4);
    assert_eq!(ui.raw_read_record().unwrap().raw.as_ref().unwrap().frame.ch[0][280], 0.5);
    assert_eq!(ui.measurement_record().unwrap().raw.as_ref().unwrap().frame.ch[0][280], 0.5);
    let zoom = ui.captured_record_view(rec, (10e-9, 0.0)).unwrap();
    assert!((zoom.ch[0][1800] - 0.5).abs() < 1e-6);
    let counters = drawpipe::counters().records_text();
    assert!(counters.contains("records read=4 deposited=4 shown=4"), "{counters}");
    ui.m.run = RunState::Stop;
    ui.step(0.04);
    assert!((ui.last[0].as_ref().unwrap().ch[0][1800] - 0.5).abs() < 1e-6);
}

#[test]
fn batch_average_preserves_legacy_settings_count_clear_and_run_seeding() {
    let _scope = drawpipe::test_scope::Scope::new();
    let mut ui = ui();
    let close = |actual: f32, expected: f32| assert!((actual - expected).abs() < 1e-6, "{actual} != {expected}");
    close(advance(&mut ui, 0, &[4, 8]), 0.375);
    ui.m.trig_level += 0.1;
    ui.step(0.04);
    close(advance(&mut ui, 2, &[8]), 0.4375);
    ui.m.avg_count = 4;
    ui.step(0.04);
    close(advance(&mut ui, 3, &[8]), 0.453125);

    // Clear invalidates both the UI seed and any published old-epoch batch.
    submit(&ui, 4, &[12, 14]);
    ui.clear_display();
    ui.step(0.04);
    close(advance(&mut ui, 6, &[4, 8]), 0.3125);
    let counters = drawpipe::counters().records_text();
    assert!(counters.contains("records read=8 deposited=6 shown=6"), "{counters}");
    assert!(counters.contains("lost.settings=2 "), "{counters}");

    ui.m.run = RunState::Stop;
    ui.step(0.04);
    ui.m.run = RunState::Run;
    ui.step(0.04);
    close(advance(&mut ui, 8, &[12, 8]), 0.6875);
}

#[test]
fn software_average_pairs_and_incompatible_modes_keep_the_record_route() {
    let _scope = drawpipe::test_scope::Scope::new();
    let mut ui = ui();
    let pipe: &Pipe = drawpipe::pipe().unwrap();
    let id = change::acquisition::identity(&ui);
    let (n, window, _) = crate::board::record::hw_grid(&ui.m);
    assert!(average::geometry_ok(pipe, id, window / n as f64, false));
    assert!(!average::geometry_ok(pipe, id, window / n as f64 / 2.0, false));
    assert!(!average::geometry_ok(pipe, id, window / n as f64, true));
    let raw = Arc::new(Frame {
        dt: window / n as f64 / 4.0,
        pre: 7_500,
        ch: [vec![0.25; 15_000], Vec::new(), Vec::new(), Vec::new()],
        ..Default::default()
    });
    let mut pair = crate::wave::hw_record(&ui.m, raw);
    assert!(pair.pairs);
    blend_record_average(&ui, &mut pair, FirstDraw::Keep);
    let counters = drawpipe::counters().pipeline_text(std::time::Duration::ZERO, 0);
    assert!(counters.contains("average_pairs=1 "), "{counters}");

    let model = ui.m.clone();
    let changes: [fn(&mut Model); 3] = [|m| m.avg_hw = true, |m| m.segments = 4, |m| m.trig_mode = TrigMode::Single];
    for change in changes {
        ui.m = model.clone();
        change(&mut ui.m);
        assert!(!ui.fast_ok());
    }
}

#[test]
fn warmed_average_picture_reuses_screen_and_history_storage() {
    let _scope = drawpipe::test_scope::Scope::new();
    let mut ui = ui();
    advance(&mut ui, 0, &[4, 8]);
    let stored = Record { raw: None, ..ui.history.front().unwrap().clone() };
    ui.history.resize(HISTORY, stored);
    let pipe = drawpipe::pipe().unwrap();
    for i in 0..8 {
        submit(&ui, i + 2, &[8]);
        let (mut rec, output) = pipe.take_waveform((ui.ph.w, ui.ph.h)).unwrap().read(&ui.m);
        let output = output.unwrap();
        use crate::system::test_allocator::count_own;
        let mut first = FirstDraw::Minimum;
        let (screen, ()) = count_own(|| prepare_batch_average(&mut ui, &mut rec, &output, first));
        let (draw, _) =
            count_own(|| draw_average(&mut ui, &mut step_draw::Composition::default(), &rec, output, &mut first));
        let (history, ()) = count_own(|| {
            remember_average(&mut ui, &rec);
            store_average_screen(&mut ui, true);
            ui.last[0] = Some(rec);
        });
        if i > 1 {
            assert_eq!((screen, draw, history), (0, 0, 0), "screen transform, draw, and history");
        }
    }
}
