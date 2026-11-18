// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::drawpipe::{Accepted, Pipe, Stats};
use std::time::{Duration, Instant};

#[derive(Default)]
pub(super) struct BatchPicture {
    pub(super) frames: u32,
    window: crate::picture::DepositWindow,
}

pub(super) fn reset_window(ui: &mut Ui) {
    if let Some(pipe) = crate::drawpipe::pipe(crate::drawpipe::Access::Existing) {
        pipe.discard_window(&mut ui.batch_picture.window);
    }
}

pub(super) fn window_time(ui: &mut Ui, before: f64) -> Option<Instant> {
    if PERSIST[ui.m.persist].1 == 0.0
        && ui.m.run == RunState::Run
        && !ui.record_mode()
        && ui.m.draw_batch
        && ui.fast_ok()
    {
        Some(ui.host.picture_time(before))
    } else {
        reset_window(ui);
        None
    }
}

fn merge(ui: &mut Ui, pipe: &Pipe, clear: &mut bool, now: Option<Instant>) -> Accepted {
    let window = now.map(|now| (&mut ui.batch_picture.window, now));
    let stats = Stats { edges: &mut ui.fast_edges, off: &mut ui.fast_off };
    pipe.accept_into(&mut ui.ph, clear, stats, window)
}

fn accept(ui: &mut Ui, d: Accepted, composition: &mut Composition) -> Option<u64> {
    let accounted = d.frames > 0 || d.age_rejected;
    let newest = deep_draw::newest(ui, d.waveform).filter(|_| accounted);
    if d.triggered > 0 {
        ui.m.status = TrigStatus::Triggered;
    }
    composition.merged += d.frames;
    (composition.deposit, composition.newest) = (composition.deposit + d.frames as usize, d.newest);
    crate::pacing::drawn_stale(d.stale);
    ui.mstat_fold(&d.folds);
    newest
}

#[derive(Default)]
pub(super) struct Fast {
    pub(super) frames: usize,
    pub(super) newest: Option<u64>,
    pub(super) raw: Option<Record>,
    pub(super) window_frames: Option<u64>,
    pub(super) ms: f32,
}

pub(super) fn fast(
    ui: &mut Ui,
    sw: &crate::sysconf::Switches,
    clear: &mut bool,
    window_now: Option<Instant>,
    composition: &mut Composition,
    draw: &mut [Duration; 5],
) -> Fast {
    let started = Instant::now();
    let frames = ui.src.drain_newest(ui.fast_max());
    let before_merge = Instant::now();
    let merged = crate::drawpipe::pipe(sw).map(|pipe| merge(ui, pipe, clear, window_now));
    let after_merge = Instant::now();
    if !frames.is_empty() && std::mem::take(clear) {
        reset_window(ui);
        ui.ph.clear();
    }
    let mut result = Fast { frames: frames.len(), ..Default::default() };
    for f in &frames {
        crate::pacing::drawn(f.cfg, 1);
    }
    composition.deposit += result.frames;
    let deep_raw = crate::deepmeas::want(&ui.m).is_some();
    if let Some(f) = frames.iter().rev().find(|f| deep_raw && crate::deepmeas::deep_raw_frame(f)) {
        let t = Instant::now();
        result.raw = Some(crate::wave::hw_record(&ui.m, f.clone()));
        crate::deepmeas::cost::set(&crate::deepmeas::cost::RECORD_US, t.elapsed());
    }
    if result.frames > 0 && ui.fast_draw_all(frames) > 0 {
        ui.m.status = TrigStatus::Triggered;
    }
    draw[1] += before_merge - started + after_merge.elapsed();
    let ms = draw[1].as_secs_f32() * 1e3;
    ui.t_fast += 0.05 * (ms - ui.t_fast);
    if result.frames > 0 && ms > 0.0 {
        ui.fast_take = fast_take_next(ui.fast_take, result.frames, ms, ui.fast_budget());
    }
    let processing = Instant::now();
    if let Some(d) = merged {
        result.window_frames = d.window_frames;
        result.frames += d.frames as usize;
        result.newest = accept(ui, d, composition);
    }
    draw[2] += after_merge - before_merge + processing.elapsed();
    let merge_ms = draw[2].as_secs_f32() * 1e3;
    result.ms = ms + merge_ms;
    crate::pacing::fast_block(ms, merge_ms);
    result
}

pub(super) fn stopped(ui: &mut Ui) {
    // stopped: the whole stored record for the view shown (read again by the thread, `stop_replay`), then
    // redraw the stored record with the current scale/offset
    ui.stop_replay();
    ui.ph.clear();
    ui.phz.clear();
    let mainv = (ui.m.time_per_div(), ui.m.delay);
    let zv = (tscales()[ui.m.zscale], ui.m.zdelay);
    let zoom_model = ui.zoom_display_model();
    let detail = ui.captured_view(zv);
    // the view read again as pairs while the raw record of the STOP is the one measured: the pairs are drawn
    let shown = ui.stop_picture().cloned().or_else(|| ui.last[0].clone());
    if let Some(rec) = shown {
        let zw = ui.z_weights(&rec);
        for c in 0..NCH {
            if ui.m.ch[c].on && !rec.ch[c].is_empty() && ui.m.z_src != Some(c) {
                let shifted_main = ui.deskew_channel_view(&rec, c, mainv);
                let main = shifted_main.as_deref().unwrap_or(&rec);
                ui.ph.draw_ex(
                    c,
                    &ui.m,
                    mainv,
                    main.t0 - ui.m.deskew_ch[c],
                    main.dt,
                    &main.ch[c],
                    24 * 6,
                    if shifted_main.is_some() { None } else { zw.as_deref() },
                    main.pairs,
                );
                if ui.m.zoom {
                    let shifted = ui.deskew_channel_view(&rec, c, zv);
                    let z = shifted.as_deref().or(detail.as_deref()).unwrap_or(&rec);
                    ui.phz.draw_ex(
                        c,
                        &zoom_model,
                        zv,
                        z.t0 - ui.m.deskew_ch[c],
                        z.dt,
                        &z.ch[c],
                        24 * 6,
                        if shifted.is_some() || detail.is_some() { None } else { zw.as_deref() },
                        z.pairs,
                    );
                }
            }
        }
    }
}

pub(super) fn render_start(ui: &mut Ui, l: &mut Layers) -> bool {
    ui.relayout();
    if let Some(o) = l.over.as_deref_mut() {
        o.no_clip();
        o.clear(0);
    }
    if ui.start_up_active() {
        ui.draw_start_up(l.under);
        // The power dialog remains over the start-up screen.
        ui.draw_overlay(l.over());
        return true;
    }
    false
}

pub(crate) fn route(ui: &Ui) -> &'static str {
    if ui.record_mode() {
        return "replay";
    }
    if ui.m.run == RunState::Stop {
        return "stopped";
    }
    if !ui.fast_ok() {
        return "records";
    }
    if ui.m.draw_batch {
        "batch"
    } else {
        "standard"
    }
}

#[derive(Default)]
pub(super) struct Composition {
    pub(super) deposit: usize,
    pub(super) record: usize,
    pub(super) newest: Option<std::time::Instant>,
    pub(super) merged: u64,
}

pub(super) fn account_picture(ui: &Ui, records: usize, composition: Composition) {
    let Composition { deposit, record, newest, merged } = composition;
    let counters = crate::drawpipe::counters();
    crate::drawpipe::Counters::add(&counters.used, records as u64);
    if ui.src.is_hw() && counters.diagnostics_enabled() {
        crate::drawpipe::Counters::add(&counters.shown, records as u64);
        crate::drawpipe::Counters::add(&counters.deposited, records as u64 - merged);
    }
    crate::pacing::composition(deposit, record, newest.map(|t| t.elapsed().as_secs_f32() * 1e3));
    crate::pacing::picture(records);
}

pub(super) fn timing(ui: &mut Ui, parts: [std::time::Duration; 5], records: usize) {
    let timing = change::timing_mut(ui);
    timing.draw_step(parts);
    if crate::drawpipe::counters().diagnostics_enabled() {
        timing.picture_frames(records);
    }
}
