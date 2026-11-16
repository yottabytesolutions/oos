// SPDX-License-Identifier: GPL-2.0-or-later

//! Run consumers act once on the accepted change, before acquisition publication.

use super::*;

pub(super) fn apply(ui: &mut Ui) -> bool {
    let Some(prepared) = ui.host.runtime().change.prepared.as_ref() else {
        // Keep servicing the last accepted run and any outstanding final read on rejection.
        // New run effects, average/history resets and final-read requests need acceptance.
        let final_done = ui.src.hw_hold(!ui.was_running && (ui.last[0].is_some() || ui.cleared), ui.final_req);
        if ui.seg_hw_on() || ui.seg_view.is_some() {
            ui.seg_poll(ui.was_running && ui.seg_hw_on());
        }
        if !ui.was_running {
            ui.cancel_demo_acquisition();
        }
        return final_done;
    };
    let (running, run_edge, single) = (!prepared.stopped, prepared.run_edge, prepared.single);
    let id = prepared.changes.identity();
    let pair_changed = ui
        .host
        .runtime()
        .change
        .last
        .as_ref()
        .is_none_or(|c| (c.identity().gen(), c.identity().epoch()) != (id.gen(), id.epoch()));
    let segment_boundary = running && ui.seg_hw_on() && (single || pair_changed);
    let segment_history = segment_boundary && prepared.changes.classes().contains(C::HISTORY);
    if run_edge || single {
        restart_average(ui);
    }
    if (run_edge || segment_history) && ui.m.segments > 0 {
        restart_history(ui);
    } else if segment_boundary {
        restart_segments(ui);
    }
    if !running && ui.was_running && ui.stop_wants_final() {
        ui.final_req += 1;
        ui.stop_final = Some(ui.now + STOP_FINAL_WAIT);
    }
    let final_done = ui.src.hw_hold(!running && (ui.last[0].is_some() || ui.cleared), ui.final_req);
    ui.was_running = running;
    if ui.seg_hw_on() || ui.seg_view.is_some() {
        ui.seg_poll(running && ui.seg_hw_on());
    }
    // stopped: a full-depth demo capture still under way is dropped
    if !running {
        ui.cancel_demo_acquisition();
    }
    final_done
}

fn restart_average(ui: &mut Ui) {
    // The other HW_AVERAGE consumers stay at their legacy seams until e.
    if ui.src.is_hw() {
        ui.m.hw_avg_clear += 1;
        if let Some(p) = &mut ui.host.runtime_mut().change.prepared {
            let row = if p.single { Row::Single } else { Row::Run };
            p.signals.effects |= C::HW_AVERAGE;
            p.signals.effects_by_row[row as usize] |= C::HW_AVERAGE;
            p.signals.rows |= 1 << row as usize;
        }
    }
}

fn restart_history(ui: &mut Ui) {
    if let Some(p) = &mut ui.host.runtime_mut().change.prepared {
        p.signals.effects |= C::HISTORY;
        p.signals.effects_by_row[Row::Run as usize] |= C::HISTORY;
    }
    ui.seg_n = 0;
    ui.history.clear();
    ui.hist_pos = None;
    if ui.seg_hw_on() {
        restart_segments(ui);
    }
}

fn restart_segments(ui: &mut Ui) {
    ui.seg_run += 1;
    ui.seg_n = 0;
    (ui.seg_pos, ui.seg_view) = (None, None);
    ui.seg_times.clear();
}

pub(super) fn prepare(ui: &mut Ui) -> bool {
    let queued = ui.host.runtime().change.pending.entries().contains(&Command::Request(Request::Run));
    let boundary = ui.m.run == RunState::Run && (!ui.was_running || queued);
    if boundary && !queued {
        request(ui, Request::Run, Row::Run);
    }
    boundary
}
