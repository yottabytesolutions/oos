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

fn decode(value: u8, f: &mut Frame) {
    f.dt = 2e-9;
    f.pre = 280;
    f.ch[0].clear();
    f.ch[0].resize(560, f32::from(value) / 16.0);
}

fn frame(ui: &Ui, seq: u64, code: u8) -> Frame {
    let id = change::acquisition::identity(ui);
    let mut frame = Frame { seq, cfg: id.gen(), stamp: id.epoch(), ..Default::default() };
    decode(code, &mut frame);
    frame
}

fn queue_record(ui: &Ui, seq: u64, code: u8) {
    let Source::Hw { shared, .. } = &ui.src else { panic!("requires scripted hardware") };
    crate::perfguard::deep::ui_lock(shared).queue.push_back(Arc::new(frame(ui, seq, code)));
    Counters::add(&drawpipe::counters().acquired, 1);
}

fn submit(ui: &Ui, first: usize, codes: &[u8]) {
    submit_with(ui, first, codes, decode);
}

fn submit_with(ui: &Ui, first: usize, codes: &[u8], fill: fn(u8, &mut Frame)) {
    let id = change::acquisition::identity(ui);
    let code = *codes.last().unwrap();
    let mut newest = frame(ui, (first + codes.len()) as u64, code);
    fill(code, &mut newest);
    let pipe = drawpipe::pipe().unwrap();
    Counters::add(&pipe.batch_counters().acquired, codes.len() as u64);
    assert!(pipe
        .feed_prepared(
            Box::new(Codes(codes.to_vec())),
            1,
            0..codes.len(),
            (id, std::time::Instant::now()),
            Arc::new(move |_, bytes, f| fill(bytes[0], f)),
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
fn batch_average_counts_every_fold_and_retains_the_newest_raw_for_raw_readout() {
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
    assert_eq!(ui.measurement_record().unwrap().ch[0][1800], value);
    assert!(ui.reader_capture_view(rec, (10e-9, 0.0)).is_none());
    let counters = drawpipe::counters().records_text();
    assert!(counters.contains("records read=4 deposited=4 shown=4"), "{counters}");
    ui.m.run = RunState::Stop;
    ui.step(0.04);
    assert_eq!(ui.last[0].as_ref().unwrap().ch[0][1800], value);
}

#[test]
fn batch_average_reseeds_for_trigger_clear_and_run_but_keeps_count_changes() {
    let _scope = drawpipe::test_scope::Scope::new();
    let mut ui = ui();
    let close = |actual: f32, expected: f32| assert!((actual - expected).abs() < 1e-6, "{actual} != {expected}");
    close(advance(&mut ui, 0, &[4, 8]), 0.375);
    ui.m.trig_level += 0.1;
    ui.step(0.04);
    close(advance(&mut ui, 2, &[12]), 0.75);
    ui.m.avg_count = 4;
    ui.step(0.04);
    close(advance(&mut ui, 3, &[8]), 0.6875);

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
    blend_record_average(&mut ui, &mut pair, FirstDraw::Keep);
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
        let tap = pipe.take_waveform((ui.ph.w, ui.ph.h)).unwrap();
        let eligibility = tap.eligibility();
        let (rec, output) = tap.read(&ui.m);
        let mut first = FirstDraw::Minimum;
        let mut taps = deep_draw::ReaderTaps::new(&ui);
        let mut composition = step_draw::Composition::default();
        let (allocations, accepted) = crate::system::test_allocator::count_own(|| {
            consume_batch(&mut ui, (rec, eligibility, output.unwrap()), &mut composition, &mut first, &mut taps)
        });
        assert_eq!(accepted.unwrap().0, 1);
        if i > 1 {
            assert_eq!(allocations, 0, "screen transform, draw, and history");
        }
    }
}

#[test]
fn hardware_average_preserves_fallback_counters_even_for_a_non_fpga_record() {
    let _scope = drawpipe::test_scope::Scope::new();
    let mut ui = ui();
    ui.m.avg_hw = true;
    ui.step(0.04);
    queue_record(&ui, 1, 4);
    ui.step(0.04);
    let counters = drawpipe::counters().pipeline_text(std::time::Duration::ZERO, 0);
    assert!(counters.contains("average_pairs=0 direct_record=0"), "{counters}");
    assert_eq!(ui.records, 1);
}

#[test]
fn single_epoch_rejects_a_queued_average_tap() {
    let _scope = drawpipe::test_scope::Scope::new();
    let mut ui = ui();
    submit(&ui, 0, &[4, 8]);
    let pipe = drawpipe::pipe().unwrap();
    assert_eq!(pipe.tap_count(), 1);
    let before = change::acquisition::identity(&ui);

    ui.key(key::SINGLE);
    ui.step(0.04);

    let armed = change::acquisition::identity(&ui);
    assert_eq!(armed.gen(), before.gen());
    assert_eq!(armed.epoch(), before.epoch() + 1);
    assert_eq!(ui.m.run, RunState::Run);
    assert_eq!(ui.m.trig_mode, TrigMode::Single);
    assert!(ui.last[0].is_none());
    assert_eq!(pipe.tap_count(), 0);
    let counters = drawpipe::counters().records_text();
    assert!(counters.contains("lost.settings=2 "), "{counters}");

    queue_record(&ui, 3, 12);
    ui.step(0.04);
    let record = ui.last[0].as_ref().unwrap();
    assert_eq!(record.raw.as_ref().unwrap().frame.seq, 3);
    assert_eq!(record.ch[0][1800], 0.75);
}

#[test]
fn average_grid_changes_reseed_on_both_routes_but_vertical_mapping_keeps_the_seed() {
    for batch in [true, false] {
        let _scope = drawpipe::test_scope::Scope::new();
        let mut ui = ui();
        ui.m.mask_on = !batch;
        ui.step(0.04);
        let mut seq = 0;
        let next = |ui: &mut Ui, seq: &mut usize, code| {
            assert_eq!(ui.fast_ok(), batch);
            if batch {
                advance(ui, *seq, &[code])
            } else {
                queue_record(ui, (*seq + 1) as u64, code);
                ui.step(0.04);
                ui.last[0].as_ref().unwrap().ch[0][1800]
            }
        };
        let mut check_next = |ui: &mut Ui, code: u8, expected: f32| {
            let actual = next(ui, &mut seq, code);
            assert!((actual - expected).abs() < 1e-6, "batch={batch} seq={seq} {actual} != {expected}");
            seq += 1;
        };

        check_next(&mut ui, 4, 0.25);
        ui.m.ch[0].vscale += 1;
        ui.m.ch[0].offset += 0.01;
        ui.step(0.04);
        check_next(&mut ui, 8, 0.375);

        let settings: [fn(&mut Model); 7] = [
            |m| m.tscale += 1,
            |m| m.delay += 5e-9,
            |m| m.zoom = true,
            |m| m.zdelay += 5e-9,
            |m| m.mdepth = 1,
            |m| m.slope = Slope::Fall,
            |m| m.ch[1].on = true,
        ];
        for change in settings {
            check_next(&mut ui, 4, 0.3125);
            change(&mut ui.m);
            ui.step(0.04);
            check_next(&mut ui, 6, 0.375);
        }
    }
}

#[test]
fn filtered_average_batch_is_a_selection_loss_without_changing_the_seed() {
    let _scope = drawpipe::test_scope::Scope::new();
    let mut ui = ui();
    assert_eq!(advance(&mut ui, 0, &[4]), 0.25);
    submit(&ui, 1, &[14, 16]);
    let (rec, output) = drawpipe::pipe().unwrap().take_waveform((ui.ph.w, ui.ph.h)).unwrap().read(&ui.m);
    let mut taps = deep_draw::ReaderTaps::new(&ui);
    let mut composition = step_draw::Composition::default();
    let mut first = FirstDraw::Replace;
    ui.m.z_src = Some(0);
    assert!(consume_batch(
        &mut ui,
        (rec, PictureEligibility::Drawable, output.unwrap()),
        &mut composition,
        &mut first,
        &mut taps,
    )
    .is_none());
    assert_eq!(composition.filtered, 0);
    ui.m.z_src = None;
    assert_eq!(advance(&mut ui, 3, &[12]), 0.5);
    let counters = drawpipe::counters();
    assert_eq!(counters.read(), (4, 2, 2));
    let text = counters.records_text();
    assert!(text.contains("records read=4 deposited=2 shown=2"), "{text}");
    assert!(text.contains("lost.settings=0 "), "{text}");
    assert!(text.contains("lost.average=2 "), "{text}");
}

#[test]
fn first_average_batch_only_reuses_a_compatible_normal_record() {
    for change_grid in [false, true] {
        let _scope = drawpipe::test_scope::Scope::new();
        let mut ui = ui();
        ui.m.acq = AcqMode::Normal;
        ui.step(0.04);
        queue_record(&ui, 1, 4);
        ui.step(0.04);
        assert_eq!(ui.last[0].as_ref().unwrap().ch[0][1800], 0.25);

        ui.m.acq = AcqMode::Average;
        if change_grid {
            ui.m.tscale += 1;
            ui.m.ch[1].on = true;
        }
        ui.step(0.04);
        let expected = if change_grid { 0.5 } else { 0.375 };
        let actual = advance(&mut ui, 1, &[8]);
        assert!((actual - expected).abs() < 1e-6, "change_grid={change_grid} {actual} != {expected}");
    }
}

mod readers;
