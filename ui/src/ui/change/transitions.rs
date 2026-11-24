// SPDX-License-Identifier: GPL-2.0-or-later

//! Run consumers act once on the accepted change, before acquisition publication.

use super::*;

pub(in crate::ui) fn wants_final_record(ui: &Ui, frame: Option<&crate::board::Frame>) -> bool {
    let m = &ui.m;
    ui.src.hw_serial()
        || frame.is_some_and(|f| {
            crate::compress::live(f) || (crate::deepmeas::want(m).is_some() && crate::deepmeas::deep_pairs(f))
        })
        || m.trig_mode == TrigMode::Auto
            && (m.la_on() || frame.is_some_and(|f| f.acq.1 > crate::compress::MAX_DEPTH as usize))
}

pub(super) fn apply(ui: &mut Ui) -> bool {
    let Some(prepared) = ui.host.runtime().change.prepared.as_ref() else {
        // Keep servicing the last accepted run and any outstanding final read on rejection.
        // New run effects, average/history resets and final-read requests need acceptance.
        let final_done = ui.src.hw_hold(!ui.was_running && (ui.last[0].is_some() || ui.cleared), ui.final_req);
        if ui.seg_hw_on() || ui.seg_view.is_some() || ui.seg_pos.is_some() {
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
    if segment_boundary || run_edge && ui.seg_hw_on() {
        restart_segments(ui);
    } else if running && single && ui.m.segments > 0 {
        super::consumers::apply_classes(ui, C::HISTORY);
    }
    let hold = !running && (ui.last[0].is_some() || ui.cleared);
    let mut final_done = ui.src.hw_hold(hold, ui.final_req);
    // Freeze the source before checking whether a serial capture can still arrive.
    if hold && ui.was_running && ui.stop_wants_final() {
        ui.final_req += 1;
        ui.stop_final = Some(ui.now + STOP_FINAL_WAIT);
        final_done = ui.src.hw_hold(hold, ui.final_req);
    }
    ui.was_running = running;
    if ui.seg_hw_on() || ui.seg_view.is_some() || ui.seg_pos.is_some() {
        ui.seg_poll(running && ui.seg_hw_on());
    }
    // stopped: a full-depth demo capture still under way is dropped
    if !running {
        ui.cancel_demo_acquisition();
    }
    final_done
}

fn restart_segments(ui: &mut Ui) {
    ui.seg_run += 1;
    // HISTORY already reset the stores before polling; pair-only restarts retain ordinary history.
    if ui.host.runtime().change.prepared.as_ref().is_some_and(|p| p.changes.classes().contains(C::HISTORY)) {
        return;
    }
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
