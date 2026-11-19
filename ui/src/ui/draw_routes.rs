// SPDX-License-Identifier: GPL-2.0-or-later

//! M12a characterization of the current routes, including shared departures from the target spec.

use super::*;
use crate::board::scripted::{self, Batch, BatchKind, BatchSource, ScriptBytes, ScriptedBatchSource};
use crate::drawpipe::{self, test_scope::Scope, RawBatch};
use oos_hal::sync::lock;
use std::sync::Arc;

use crate::system::test_allocator::Counting;
#[global_allocator]
static HEAP: Counting = Counting::new();
mod counters;
mod funding;
mod perf;
mod tap_recovery;

struct Route {
    ui: Ui,
    source: ScriptedBatchSource,
    spare: Option<ScriptBytes>,
    records: bool,
}

impl Route {
    fn new(kind: BatchKind, batch: bool, channels: usize, tdiv: f32) -> Self {
        let mut ui = Ui::new(Source::Hw { shared: Default::default(), seen: 0 }, host::Host::headless());
        ui.m.draw_batch = batch;
        ui.m.auto_roll = kind == BatchKind::Roll;
        ui.m.meas.clear();
        ui.m.meas2.clear();
        ui.m.tscale = tscales().iter().position(|&t| (t - tdiv).abs() < tdiv * 1e-4).unwrap();
        ui.m.dots = DotsMode::Off;
        for (c, ch) in ui.m.ch.iter_mut().enumerate() {
            ch.on = c < channels;
            ch.offset = 0.0;
        }
        ui.relayout();
        Self {
            ui,
            source: scripted::source(kind, 123),
            spare: None,
            records: matches!(kind, BatchKind::Roll | BatchKind::Scan),
        }
    }

    fn step(&mut self, frames: usize) {
        let bytes = self.dispatch(frames, |route, sw| {
            route.source.next_batch(&route.ui.m, sw, frames, route.spare.take()).unwrap()
        });
        self.spare = Some(*bytes.into_any().downcast().unwrap());
    }

    fn dispatch<B: Batch>(
        &mut self,
        frames: usize,
        fixture: impl FnOnce(&mut Self, &crate::sysconf::Switches) -> B,
    ) -> Box<dyn RawBatch>
    where
        B::Bytes: Into<Box<dyn RawBatch>>,
    {
        // Fixed intake count and injected dt: elapsed draw time cannot alter picture membership.
        self.ui.fast_take = 48.0;
        let id = change::acquisition::script_prepare_route(&mut self.ui, self.records);
        let sw = change::acquisition::draw_switches(&self.ui, &self.ui.m);
        let batch = fixture(self, &sw);
        let Source::Hw { shared, .. } = &self.ui.src else { unreachable!() };
        let tag = |mut f: crate::board::Frame| {
            (f.cfg, f.stamp) = (id.gen(), id.epoch());
            Arc::new(f)
        };
        let pipe = drawpipe::pipe(&sw).filter(|p| self.ui.m.draw_batch && p.accepting());
        let bytes = if let Some(pipe) = pipe {
            let newest = tag(batch.newest(id.epoch()));
            let (fb, decode, codes) = (batch.frame_bytes(), batch.decode(), batch.codes());
            let at = self.ui.host.picture_time(self.ui.now);
            pipe.feed_at(batch.into_bytes().into(), fb, 0..frames, (id, at), decode, codes, Some(newest));
            drawpipe::host::finish_batch(pipe, 1).unwrap()
        } else {
            let mut g = lock(shared);
            g.queue.extend((0..frames).map(|i| tag(batch.frame(i, id.epoch()))));
            g.frame = Arc::clone(g.queue.back().unwrap());
            batch.into_bytes().into()
        };
        self.ui.step(1.0 / 30.0);
        bytes
    }
}

impl From<ScriptBytes> for Box<dyn RawBatch> {
    fn from(bytes: ScriptBytes) -> Self {
        Box::new(bytes)
    }
}

impl RawBatch for Box<dyn RawBatch> {
    fn bytes(&self) -> &[u8] {
        (**self).bytes()
    }

    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any + Send> {
        (*self).into_any()
    }
}

#[derive(Debug, PartialEq)]
struct Picture {
    main: [Vec<u16>; NCH],
    zoom: [Vec<u16>; NCH],
    frames: u32,
}

fn picture(ui: &Ui) -> Picture {
    Picture { main: ui.ph.maps().clone(), zoom: ui.phz.maps().clone(), frames: ui.batch_picture.frames }
}

fn weights(p: &Picture) -> [u64; NCH] {
    p.main.each_ref().map(|map| map.iter().map(|&v| u64::from(v)).sum())
}

fn compare(a: &Picture, b: &Picture, label: &str) {
    assert_eq!(a.frames, b.frames, "{label}: counted frames");
    assert_eq!(weights(a), weights(b), "{label}: total deposited weight");
    assert!(a.main == b.main && a.zoom == b.zoom, "{label}: pixel maps");
    assert!(weights(a)[0] > 0, "{label}: nonempty negative control");
}

#[test]
fn batch_redraws_current_waveform_after_mapping_rejects_its_planes() {
    let _scope = Scope::new();
    let mut route = Route::new(BatchKind::Codes, true, 1, 10e-6);
    route.step(1);
    assert!(route.ui.ph.maps()[0].iter().any(|&v| v > 0));

    let id = change::acquisition::identity(&route.ui);
    let sw = change::acquisition::draw_switches(&route.ui, &route.ui.m);
    let pipe = drawpipe::pipe(&sw).unwrap();
    let batch = route.source.next_batch(&route.ui.m, &sw, 1, route.spare.take()).unwrap();
    let mut waveform = batch.newest(id.epoch());
    waveform.cfg = id.gen();
    let waveform = Arc::new(waveform);
    let (fb, decode, codes) = (batch.frame_bytes(), batch.decode(), batch.codes());
    let at = route.ui.host.picture_time(route.ui.now);
    pipe.feed_at(batch.into_bytes().into(), fb, 0..1, (id, at), decode, codes, Some(waveform.clone()));
    drawpipe::host::finish_batch(pipe, 2).unwrap();

    route.ui.m.dots = DotsMode::On;
    route.ui.step(1.0 / 30.0);

    assert!(drawpipe::counters().records_text().contains("rejected.stale_key=1 "));
    let shown = route.ui.last[0].as_ref().unwrap().raw.as_ref().unwrap();
    assert_eq!(shown.frame.seq, waveform.seq);
    let mut expected = Phosphor::new(route.ui.ph.w, route.ui.ph.h);
    drawpipe::draw_frame(&mut expected, &route.ui.m, &waveform, sw.interp, None);
    assert!(expected.maps()[0].iter().any(|&v| v > 0));
    assert!(route.ui.ph.maps() == expected.maps(), "the delivered waveform should be drawn with dots");
    assert_eq!(route.ui.batch_picture.frames, 1);
}

#[test]
fn batch_draws_newest_waveform_after_hit_budget_rejection() {
    let _scope = Scope::new();
    let mut route = Route::new(BatchKind::Raw, true, 1, 10e-6);
    route.ui.m.dots = DotsMode::On;
    let id = change::acquisition::script_prepare(&mut route.ui);
    let sw = change::acquisition::draw_switches(&route.ui, &route.ui.m);
    let pipe = drawpipe::pipe(&sw).unwrap();
    let frames = 32;
    let batch = route.source.next_batch(&route.ui.m, &sw, frames, None).unwrap();
    let mut waveform = batch.newest(id.epoch());
    waveform.cfg = id.gen();
    // A hundred samples per column fit for one waveform but exceed the batch hit budget.
    let samples = route.ui.ph.w * 100;
    waveform.dt = f64::from(route.ui.m.time_per_div()) * f64::from(HDIV) / samples as f64;
    waveform.pre = samples / 2;
    waveform.ch[0] = vec![0.25; samples];
    let waveform = Arc::new(waveform);
    let captured = waveform.clone();
    let decode = Arc::new(move |_: &[u8], frame: &mut crate::board::Frame| frame.clone_from(&captured));
    let fb = batch.frame_bytes();
    let at = route.ui.host.picture_time(route.ui.now);
    pipe.feed_at(batch.into_bytes().into(), fb, 0..frames, (id, at), decode, None, Some(waveform.clone()));
    drawpipe::host::finish_batch(pipe, 1).unwrap();
    assert!(drawpipe::counters().records_text().contains("lost.budget=32 "));

    route.ui.step(1.0 / 30.0);

    let mut expected = Phosphor::new(route.ui.ph.w, route.ui.ph.h);
    drawpipe::draw_frame(&mut expected, &route.ui.m, &waveform, sw.interp, None);
    assert!(expected.maps()[0].iter().any(|&v| v > 0));
    assert!(route.ui.ph.maps() == expected.maps(), "the newest waveform should populate the picture");
    assert_eq!(route.ui.batch_picture.frames, 1);
}

#[test]
fn batch_picture_keeps_recent_deposits_without_recounting_their_records() {
    let _scope = Scope::new();
    let mut route = Route::new(BatchKind::Codes, true, 1, 1e-6);
    route.step(27);
    assert!(!route.ui.ph.published_rows().is_empty());
    route.step(3);
    assert_eq!(route.ui.batch_picture.frames, 30);
    route.step(3);
    assert_eq!(route.ui.batch_picture.frames, 33);
    route.step(3);
    assert_eq!(route.ui.batch_picture.frames, 9);
    assert_eq!(route.ui.wfm_total, 36);
    assert!(route.ui.ph.maps()[0].iter().any(|&v| v > 0));

    for _ in 0..3 {
        route.ui.step(1.0 / 30.0);
    }
    assert_eq!(route.ui.batch_picture.frames, 0);
    assert_eq!(route.ui.wfm_total, 36);
    assert!(route.ui.ph.maps()[0].iter().all(|&v| v == 0));
}

#[test]
fn batch_waveform_delayed_by_a_fallback_is_measured_without_redrawing() {
    let _scope = Scope::new();
    let mut route = Route::new(BatchKind::Raw, true, 1, 1e-6);
    route.ui.m.meas = vec![(Meas::Vavg, WaveId::Ch(0))];
    let id = change::acquisition::script_prepare(&mut route.ui);
    let sw = change::acquisition::draw_switches(&route.ui, &route.ui.m);
    let pipe = drawpipe::pipe(&sw).unwrap();
    let batch = route.source.next_batch(&route.ui.m, &sw, 4, None).unwrap();
    let mut fallback = batch.frame(0, id.epoch());
    fallback.cfg = id.gen();
    fallback.ch[0].fill(0.25);
    let mut newest = batch.newest(id.epoch());
    newest.cfg = id.gen();
    newest.ch[0].fill(0.75);
    let newest = Arc::new(newest);
    let (fb, decode, codes) = (batch.frame_bytes(), batch.decode(), batch.codes());
    let at = route.ui.host.picture_time(route.ui.now);
    pipe.feed_at(batch.into_bytes().into(), fb, 1..4, (id, at), decode.clone(), codes.clone(), Some(newest.clone()));
    let buffer = drawpipe::host::finish_batch(pipe, 1).unwrap();
    let Source::Hw { shared, .. } = &route.ui.src else { unreachable!() };
    let shared = shared.clone();
    lock(&shared).queue.push_back(fallback.into());

    route.ui.step(1.0 / 30.0);
    let first = picture(&route.ui);
    assert_eq!((first.frames, route.ui.wfm_total, route.ui.history.len()), (4, 4, 1));
    assert!(weights(&first)[0] > 0);
    assert_eq!(pipe.tap_count(), 1);
    assert!((route.ui.ana_get(0, Meas::Vavg) - 0.25).abs() < 1e-5);

    route.ui.step(1.0 / 30.0);
    assert_eq!(route.ui.wfm_total, 4);
    assert_eq!(picture(&route.ui), first);
    assert_eq!(route.ui.history.len(), 2);
    assert_eq!(route.ui.last[0].as_ref().unwrap().raw.as_ref().unwrap().frame.seq, newest.seq);
    assert!((route.ui.ana_get(0, Meas::Vavg) - 0.75).abs() < 1e-5);
    assert!(Arc::ptr_eq(&lock(&shared).frame, &newest));
    assert_eq!(pipe.tap_count(), 0);
    assert!(lock(&shared).queue.is_empty());
    let samples = route.ui.mstat[0].n;
    let at = route.ui.host.picture_time(route.ui.now);
    pipe.feed_at(buffer, fb, 0..1, (id, at), decode, codes, None);
    drawpipe::host::finish_batch(pipe, 2).unwrap();
    route.ui.step(1.0 / 30.0);
    assert_eq!(route.ui.wfm_total, 5);
    assert_eq!(route.ui.mstat[0].n, samples + 1, "only the newly folded frame counts");
}

#[test]
fn auto_to_lines_transition_resets_the_batch_window() {
    let run = |transition| {
        let _scope = Scope::new();
        let mut route = Route::new(BatchKind::Raw, true, 1, 200e-6);
        route.ui.m.dots = DotsMode::Auto;
        route.step(27);
        assert_eq!(route.ui.batch_picture.frames, 27);
        if transition {
            route.ui.m.dots = DotsMode::Off;
        }
        route.step(3);
        assert_eq!(route.ui.wfm_total, 30);
        picture(&route.ui)
    };
    let auto = run(false);
    let lines = run(true);
    assert_eq!((auto.frames, lines.frames), (30, 3));
    assert_ne!(weights(&auto), weights(&lines));
    assert!(weights(&lines)[0] > 0);
}

#[test]
fn draw_routes_raw_codes_pairs_dots_and_zoom() {
    for channels in [1, 2, 4] {
        for (kind, dots, zoom) in [
            (BatchKind::Raw, false, false),
            (BatchKind::Codes, false, false),
            (BatchKind::LivePairs, false, false),
            (BatchKind::DeepPairs, false, false),
            (BatchKind::Raw, true, false),
            (BatchKind::Raw, false, true),
        ] {
            let run = |batch| {
                let _scope = Scope::new();
                let mut r =
                    Route::new(kind, batch, channels, if kind == BatchKind::LivePairs { 200e-6 } else { 10e-6 });
                r.ui.m.dots = if dots { DotsMode::On } else { DotsMode::Off };
                r.ui.m.zoom = zoom;
                r.step(3);
                assert_eq!(r.ui.wfm_total, 3);
                if zoom {
                    assert!(!r.ui.fast_ok(), "current zoom uses the shared record path");
                    assert!(r.ui.phz.maps()[0].iter().any(|&v| v > 0));
                }
                let pic = picture(&r.ui);
                if kind == BatchKind::LivePairs && !dots && !zoom && batch {
                    let mut source = scripted::source(kind, 123);
                    let b = source.next_batch(&r.ui.m, r.ui.switches(), 3, None).unwrap();
                    let mut want = crate::wave::Phosphor::new(r.ui.ph.w, r.ui.ph.h);
                    for i in 0..3 {
                        drawpipe::draw_frame(&mut want, &r.ui.m, &b.frame(i, 0), r.ui.switches().interp, None);
                    }
                    assert!(&pic.main == want.maps(), "Batch pairs use one span per column per frame");
                }
                pic
            };
            let (standard, batch) = (run(false), run(true));
            if kind == BatchKind::LivePairs {
                assert_eq!(standard.frames, batch.frames);
                assert!(weights(&standard)[0] > weights(&batch)[0]);
                println!("LivePairs {channels}ch Standard {:?}, Batch {:?}", weights(&standard), weights(&batch));
            } else {
                compare(&standard, &batch, &format!("{kind:?} {channels}ch dots={dots} zoom={zoom}"));
            }
        }
    }
}

#[test]
fn draw_routes_roll_scan_stop_single_and_replay() {
    for mode in ["roll", "scan", "stop", "single", "history", "segments"] {
        let run = |batch| {
            let _scope = Scope::new();
            let kind = match mode {
                "roll" => BatchKind::Roll,
                "scan" => BatchKind::Scan,
                _ => BatchKind::Raw,
            };
            let mut r = Route::new(kind, batch, 2, if mode == "roll" || mode == "scan" { 0.2 } else { 10e-6 });
            if mode == "history" {
                r.ui.m.zoom = true;
            }
            if mode == "single" {
                r.ui.m.trig_mode = TrigMode::Single;
            } else if mode == "segments" {
                r.ui.m.segments = 3;
                r.ui.m.seg_hw = false;
            }
            r.step(match mode {
                "single" => 1,
                "roll" | "scan" => 6,
                _ => 3,
            });
            if mode == "single" {
                r.ui.step(1.0 / 30.0);
                assert_eq!(r.ui.m.run, RunState::Stop);
            } else if mode == "stop" {
                r.ui.m.run = RunState::Stop;
            } else if mode == "history" {
                assert_eq!(r.ui.history.len(), 3);
                r.ui.history_step(-1);
                assert_eq!(r.ui.hist_pos, Some(1));
            } else if mode == "segments" {
                r.ui.step(1.0 / 30.0);
                assert_eq!(r.ui.seg_n, 3);
                assert_eq!(r.ui.m.run, RunState::Stop);
                r.ui.history_step(-1);
            }
            r.ui.step(1.0 / 30.0);
            let frozen = picture(&r.ui);
            for _ in 0..3 {
                r.ui.step(1.0 / 30.0);
                compare(&frozen, &picture(&r.ui), mode);
            }
            frozen
        };
        compare(&run(false), &run(true), mode);
    }
}

#[test]
fn draw_routes_stop_redraw_is_equal_but_does_not_freeze_the_live_picture() {
    let _scope = Scope::new();
    let mut r = Route::new(BatchKind::Raw, false, 1, 10e-6);
    r.step(3);
    let live = picture(&r.ui);
    r.ui.m.run = RunState::Stop;
    r.ui.step(1.0 / 30.0);
    let stopped = picture(&r.ui);
    assert_ne!(weights(&live), weights(&stopped), "current STOP redraw is not the specified map/W freeze");
    println!("STOP live counts {:?}, redraw counts {:?}", weights(&live), weights(&stopped));
}

#[test]
fn draw_routes_deep_raw_measurement_still_changes_the_picture() {
    let run = |batch| {
        let _scope = Scope::new();
        let mut r = Route::new(BatchKind::DeepPairs, batch, 1, 1e-3);
        r.step(3);
        let before = picture(&r.ui);
        let id = change::acquisition::script_prepare(&mut r.ui);
        let raw = crate::board::Frame {
            seq: 4,
            cfg: id.gen(),
            stamp: id.epoch(),
            pre: 5000,
            dt: 1e-6,
            acq: (1e6, 10_000_000),
            ch: [
                (0..10_000).map(|i| if i % 100 < 50 { -0.3 } else { 0.3 }).collect(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
            ],
            ..Default::default()
        };
        assert!(crate::deepmeas::deep_raw_frame(&raw));
        let Source::Hw { shared, .. } = &r.ui.src else { unreachable!() };
        {
            let mut g = lock(shared);
            let raw = Arc::new(raw);
            g.frame = raw.clone();
            g.queue.push_back(raw);
        }
        r.ui.step(1.0 / 30.0);
        let after = picture(&r.ui);
        assert_ne!(before.main, after.main, "target spec excludes this measurement read from the live picture");
        assert_eq!(after.frames, 1);
        after
    };
    compare(&run(false), &run(true), "deep raw window");
}

#[test]
fn full_picture_slot_preserves_each_coalesced_batch_for_record_readers() {
    for records in [1, 2, 4] {
        let _scope = Scope::new();
        let tdiv = if records == 1 { 1e-6 } else { 10e-6 };
        let mut route = Route::new(BatchKind::Raw, true, 1, tdiv);
        route.ui.m.meas = vec![(Meas::Vavg, WaveId::Ch(0))];
        let id = change::acquisition::script_prepare(&mut route.ui);
        let sw = change::acquisition::draw_switches(&route.ui, &route.ui.m);
        let pipe = drawpipe::pipe(&sw).unwrap();
        let mut expected = Vec::new();
        let mut samples = Vec::new();
        for batch_id in 0..3 {
            let batch = route.source.next_batch(&route.ui.m, &sw, records, None).unwrap();
            let mut newest = batch.newest(id.epoch());
            newest.cfg = id.gen();
            // Distinct owned waveforms make history order observable after history drops raw metadata.
            for value in &mut newest.ch[0] {
                *value += batch_id as f32 * 0.05;
            }
            expected.push(newest.seq);
            samples.push(crate::wave::hw_record(&route.ui.m, Arc::new(newest.clone())).ch);
            let (fb, decode, codes) = (batch.frame_bytes(), batch.decode(), batch.codes());
            let at = route.ui.host.picture_time(route.ui.now);
            assert!(pipe
                .feed_at(batch.into_bytes().into(), fb, 0..records, (id, at), decode, codes, Some(Arc::new(newest)))
                .is_none());
            // Completion returns DMA while the first deposit still occupies the UI slot.
            drawpipe::host::finish_batch(pipe, batch_id).unwrap();
        }
        route.ui.step(1.0 / 30.0);
        let received: Vec<_> = route.ui.history.iter().map(|r| r.ch.clone()).collect();
        assert_eq!(expected, [records as u64, 2 * records as u64, 3 * records as u64]);
        assert!(samples.windows(2).all(|pair| pair[0] != pair[1]));
        let observed: Vec<_> =
            received.iter().map(|ch| expected[samples.iter().position(|s| s == ch).unwrap()]).collect();
        assert_eq!(observed, expected, "real history observes every tap in FIFO order");
        assert_eq!(
            route.ui.mstat[0].n,
            records as u64 + 2,
            "the first picture contributes its siblings and all three taps"
        );
        assert_eq!(route.ui.last[0].as_ref().unwrap().raw.as_ref().unwrap().frame.seq, 3 * records as u64);
        assert_eq!(route.ui.history.len(), 3);
        drawpipe::host::barrier(pipe);
        route.ui.step(1.0 / 30.0);
        assert_eq!(route.ui.history.len(), 3, "taps consumed exactly once");
        assert_eq!(route.ui.mstat[0].n, 3 * records as u64, "all records measured once, without displaced-tap folds");
        assert_eq!(route.ui.wfm_total, 3 * records as u64, "coalesced picture records count once");
    }
}

#[test]
fn consumed_waveform_redraws_only_if_its_private_deposit_becomes_stale() {
    for remap in [false, true] {
        let _scope = Scope::new();
        let mut route = Route::new(BatchKind::Raw, true, 1, 1e-6);
        route.ui.m.meas = vec![(Meas::Vavg, WaveId::Ch(0))];
        let id = change::acquisition::script_prepare(&mut route.ui);
        let sw = change::acquisition::draw_switches(&route.ui, &route.ui.m);
        let pipe = drawpipe::pipe(&sw).unwrap();
        let mut last = None;
        for seq in 1..=2 {
            let batch = route.source.next_batch(&route.ui.m, &sw, 1, None).unwrap();
            let mut frame = batch.newest(id.epoch());
            frame.cfg = id.gen();
            frame.ch[0].fill(seq as f32 / 4.0);
            let frame = Arc::new(frame);
            let (fb, decode, codes) = (batch.frame_bytes(), batch.decode(), batch.codes());
            let at = route.ui.host.picture_time(route.ui.now);
            pipe.feed_at(batch.into_bytes().into(), fb, 0..1, (id, at), decode, codes, Some(frame.clone()));
            drawpipe::host::finish_batch(pipe, seq).unwrap();
            last = Some(frame);
        }

        route.ui.step(1.0 / 30.0);
        assert_eq!((route.ui.wfm_total, route.ui.history.len(), pipe.tap_count()), (1, 2, 0));
        assert!((route.ui.ana_get(0, Meas::Vavg) - 0.5).abs() < 1e-5);
        drawpipe::host::barrier(pipe);
        if remap {
            route.ui.m.dots = DotsMode::On;
        }
        route.ui.step(1.0 / 30.0);
        assert_eq!((route.ui.wfm_total, route.ui.history.len()), (2, 2));
        if remap {
            let mut expected = Phosphor::new(route.ui.ph.w, route.ui.ph.h);
            drawpipe::draw_frame(&mut expected, &route.ui.m, &last.unwrap(), sw.interp, None);
            assert_eq!(route.ui.ph.map, expected.map);
        }
        route.ui.step(1.0 / 30.0);
        assert_eq!((route.ui.wfm_total, route.ui.history.len()), (2, 2));
    }
}

#[test]
fn batch_steps_keep_narrow_uploads_and_idle_pictures_need_no_upload() {
    for channels in [1, 2, 4] {
        for dots in [DotsMode::Off, DotsMode::On] {
            let _scope = Scope::new();
            let mut route = Route::new(BatchKind::Codes, true, channels, 1e-6);
            route.ui.m.dots = dots;
            route.step(1);
            let stamps: [_; NCH] = std::array::from_fn(|c| route.ui.ph.upload_rows(c, None).0);
            route.step(1);
            assert!(!route.ui.ph.is_wide());
            for (c, stamp) in stamps.iter().enumerate() {
                let rows = route.ui.ph.upload_rows(c, Some(stamp)).1;
                assert_eq!(rows.is_empty(), c >= channels);
                assert!(rows.len() < route.ui.ph.h, "ordinary Batch steps keep their narrow hull");
            }
            let stamps: [_; NCH] = std::array::from_fn(|c| route.ui.ph.upload_rows(c, None).0);
            route.ui.step(0.0);
            for (c, stamp) in stamps.iter().enumerate() {
                assert!(route.ui.ph.upload_rows(c, Some(stamp)).1.is_empty());
            }
        }
    }
}
