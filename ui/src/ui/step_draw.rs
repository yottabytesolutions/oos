// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

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
