// SPDX-License-Identifier: GPL-2.0-or-later

use super::super::*;
use crate::board::{Frame, HwShared};
use oos_hal::sync::lock;
use std::sync::Arc;

fn capture(ui: &Ui, shared: &HwShared, seq: u64, volts: f32) -> Arc<Frame> {
    let id = change::acquisition::identity(ui);
    let pre = 4000;
    let dt = f64::from(ui.m.time_per_div()) * f64::from(HDIV) / 6000.0;
    let frame = Arc::new(Frame {
        seq,
        cfg: id.gen(),
        stamp: id.epoch(),
        dt,
        pre,
        fixed_ti: Some(pre as f64),
        ch: [vec![volts; 2 * pre], Vec::new(), Vec::new(), Vec::new()],
        ..Default::default()
    });
    lock(shared).queue.push_back(frame.clone());
    frame
}

fn held(worker: bool, zoom: bool, persist: usize) -> (Ui, HwShared, Arc<Frame>) {
    let shared = HwShared::default();
    let mut ui = Ui::for_tests(Source::Hw { shared: shared.clone(), seen: 0 });
    ui.m.meas.clear();
    ui.m.meas2.clear();
    ui.m.trig_mode = TrigMode::Normal;
    ui.m.ch.iter_mut().skip(1).for_each(|ch| ch.on = false);
    ui.m.acq = if worker { AcqMode::Normal } else { AcqMode::Average };
    ui.m.persist = persist;
    ui.set_zoom(zoom);
    ui.step(0.04);
    assert_eq!(ui.fast_ok(), worker);
    let old = capture(&ui, &shared, 1, 0.25);
    ui.step(0.04);
    ui.m.run = RunState::Stop;
    ui.step(0.04);
    assert!(ui.stop_final.is_none());
    (ui, shared, old)
}

pub(super) fn pixels(ui: &Ui) -> [Vec<u32>; 2] {
    std::array::from_fn(|i| {
        let ph = if i == 0 { &ui.ph } else { &ui.phz };
        let area = Rect::new(0, 0, ph.w as i32, ph.h as i32);
        let mut canvas = Canvas::new(area.w, area.h);
        ui.traces(&mut Layers { under: &mut canvas, over: None, jobs: Vec::new() }, area, i != 0, true);
        canvas.px
    })
}

fn counts(ui: &Ui) -> [Vec<u32>; 2] {
    [&ui.ph, &ui.phz].map(|ph| (0..ph.w * ph.h).map(|i| ph.count(0, i)).collect())
}

fn feed_worker(ui: &Ui) {
    use crate::board::scripted::{self, Batch, BatchKind, BatchSource};
    let mut source = scripted::source(BatchKind::Raw, 123);
    let batch = source.next_batch(&ui.m, ui.switches(), 2, None).unwrap();
    let (bytes, decode) = (batch.frame_bytes(), batch.decode());
    let pipe = crate::drawpipe::pipe().unwrap();
    let id = change::acquisition::identity(ui);
    let decode = Arc::new(move |raw: &[u8], frame: &mut Frame| {
        decode(raw, frame);
        frame.ch[0].fill(-0.25);
    });
    assert!(pipe.feed_with(Box::new(batch.into_bytes()), bytes, 0..1, id, decode, None, None).is_none());
    crate::drawpipe::host::finish_batch(pipe, 1).unwrap();
    crate::drawpipe::host::barrier(pipe);
}

#[test]
fn run_preserves_both_maps_and_replaces_on_the_first_current_contribution() {
    for _ in 0..2 {
        for (worker, zoom) in [(false, true), (true, true), (true, false)] {
            for persist in [0, 1] {
                exercise_replacement(worker, zoom, persist);
            }
        }
    }
}

fn exercise_replacement(worker: bool, zoom: bool, persist: usize) {
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let (mut ui, shared, old) = held(worker, zoom, persist);
    let previous = counts(&ui);
    let previous_pixels = pixels(&ui);
    let total = ui.wfm_total;
    assert!(previous[0].iter().any(|&v| v != 0));
    assert_eq!(previous[1].iter().any(|&v| v != 0), zoom);

    ui.m.run = RunState::Run;
    lock(&shared).queue.push_back(old.clone());
    for _ in 0..2 {
        ui.step(0.04);
        assert_eq!(ui.wfm_total, total);
        assert_eq!(counts(&ui), previous);
        assert_eq!(pixels(&ui), previous_pixels);
    }
    ui.m.intensity /= 2;
    ui.step(0.04);
    assert_eq!(counts(&ui), previous);
    assert!(pixels(&ui)[0] != previous_pixels[0], "intensity grades the waiting picture");
    ui.m.run = RunState::Stop;
    ui.step(0.04);
    assert!(Arc::ptr_eq(&ui.last[0].as_ref().unwrap().raw.as_ref().unwrap().frame, &old));

    ui.m.run = RunState::Run;
    ui.step(0.04);
    if worker {
        feed_worker(&ui);
    } else {
        capture(&ui, &shared, 2, -0.25);
    }
    ui.step(0.04);
    assert_eq!(ui.wfm_total, total + 1);
    for (old, new) in previous.into_iter().zip(counts(&ui)).take(if zoom { 2 } else { 1 }) {
        assert!(new.iter().any(|&v| v != 0));
        assert!(old.into_iter().zip(new).all(|(a, b)| a == 0 || b == 0), "old hits survive replacement");
    }
    let replaced = counts(&ui);
    ui.step(0.04);
    assert_eq!(ui.wfm_total, total + 1);
    assert_eq!(counts(&ui) == replaced, persist == 0, "ordinary persistence resumes after replacement");
}

#[test]
fn clear_and_layout_change_erase_the_waiting_picture_immediately() {
    for _ in 0..2 {
        for (zoom, edit) in [(true, 0), (true, 1), (false, 1), (false, 2)] {
            let _scope = crate::drawpipe::test_scope::Scope::new();
            let (mut ui, _, _) = held(true, zoom, 1);
            ui.m.run = RunState::Run;
            ui.step(0.04);
            assert!(ui.ph.channel_max(0) > 0);
            match edit {
                0 => ui.clear_display(),
                1 => ui.set_zoom(!zoom),
                _ => ui.m.meas.push((Meas::Vavg, WaveId::Ch(0))),
            }
            ui.step(0.04);
            assert!(counts(&ui).iter().flatten().all(|&v| v == 0));
        }
    }
}

#[test]
fn only_geometry_and_capture_changes_erase_the_waiting_picture() {
    use crate::drawpipe::test_scope::Scope;

    type Edit = fn(&mut Model);
    let changes: &[(bool, Edit)] = &[
        (true, |m| m.ch[0].on = false),
        (true, |m| m.ch[1].on = true),
        (true, |m| m.ch[0].vscale += 1),
        (true, |m| m.ch[0].offset += 0.1),
        (true, |m| m.ch[0].coupling = Coupling::Ac),
        (true, |m| m.ch[0].probe *= 10.0),
        (true, |m| m.ch[0].invert = !m.ch[0].invert),
        (true, |m| m.tscale += 1),
        (true, |m| m.delay += m.time_per_div()),
        (true, |m| m.zdelay += m.time_per_div()),
        (true, |m| m.acq = AcqMode::HiRes),
        (true, |m| m.mdepth = 1),
        (false, |m| m.trig_level += 0.1),
        (false, |m| m.trig_src = 1),
        (false, |m| m.cursor = CursorMode::Volt),
        (false, |m| m.meas[0].0 = Meas::Vrms),
    ];
    for (i, &(erase, change)) in changes.iter().enumerate() {
        let _scope = Scope::new();
        let (mut ui, _, _) = held(false, true, 1);
        ui.m.cursor = CursorMode::Time;
        ui.m.meas.push((Meas::Vavg, WaveId::Ch(0)));
        ui.step(0.04);
        ui.m.run = RunState::Run;
        ui.step(0.04);
        let before = counts(&ui);
        assert!(before[0].iter().any(|&n| n > 0));
        change(&mut ui.m);
        for _ in 0..2 {
            ui.step(0.04);
            let after = counts(&ui);
            if erase {
                assert!(after.iter().flatten().all(|&n| n == 0), "change {i}");
            } else {
                assert!(after == before, "change {i}");
            }
        }
    }
}

#[test]
fn auto_replaces_on_the_first_eligible_record() {
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let (mut ui, shared, _) = held(true, false, 1);
    let before = counts(&ui);
    let total = ui.wfm_total;
    ui.m.trig_mode = TrigMode::Auto;
    ui.m.run = RunState::Run;
    ui.step(0.04);
    assert_eq!(counts(&ui), before);
    drop(capture(&ui, &shared, 2, -0.25));
    Arc::get_mut(lock(&shared).queue.front_mut().unwrap()).unwrap().fixed_ti = None;
    ui.step(0.04);
    let after = counts(&ui);
    assert_eq!(ui.wfm_total, total + 1);
    assert_eq!(ui.m.status, TrigStatus::Auto);
    assert!(after[0].iter().any(|&n| n > 0));
    assert!(before[0].iter().zip(&after[0]).all(|(&a, &b)| a == 0 || b == 0));
}

#[test]
fn queued_prefix_keeps_waiting_for_a_drawable_record() {
    for single in [true, false] {
        let _scope = crate::drawpipe::test_scope::Scope::new();
        let (mut ui, shared, _) = held(true, false, 1);
        ui.m.run = RunState::Run;
        ui.step(0.04);
        let before = counts(&ui);
        drop(capture(&ui, &shared, 2, -0.25));
        drop(capture(&ui, &shared, 3, -0.25));
        if single {
            ui.m.trig_mode = TrigMode::Single;
            Arc::get_mut(lock(&shared).queue.front_mut().unwrap()).unwrap().partial = true;
        } else {
            ui.m.z_src = Some(0);
        }
        // Exercise the fast prefix directly. Current routing rejects SINGLE and Z input before this stage.
        let sw = ui.switches().clone();
        let mut first = crate::picture::FirstDraw::Replace;
        let fast = step_draw::fast(&mut ui, &sw, &mut first, false, &mut Default::default(), &mut Default::default());
        assert!(first.waiting());
        assert_eq!(fast.frames, 0);
        assert_eq!(counts(&ui), before);
    }
}

#[test]
fn queued_prefix_replaces_the_waiting_picture_before_the_newest_record_draws() {
    for _ in 0..2 {
        for persist in [0, 1] {
            let _scope = crate::drawpipe::test_scope::Scope::new();
            let (mut ui, shared, _) = held(true, false, persist);
            let previous = counts(&ui);
            let total = ui.wfm_total;
            ui.m.run = RunState::Run;
            ui.step(0.04);
            capture(&ui, &shared, 2, -0.25);
            capture(&ui, &shared, 3, -0.25);
            ui.step(0.04);
            assert_eq!(ui.wfm_total, total + 2);
            let current = counts(&ui);
            assert!(current[0].iter().any(|&v| v >= 48), "both new records reach the composed picture");
            assert!(previous[0].iter().zip(&current[0]).all(|(&a, &b)| a == 0 || b == 0));
        }
    }
}
