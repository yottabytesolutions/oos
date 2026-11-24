// SPDX-License-Identifier: GPL-2.0-or-later

//! Stage run consumers before publication and poll segment replies afterwards.

use super::*;

/// A plain STOP drains a serial capture or waits for the final read needed by the current view.
pub(super) fn wants_final_record(ui: &Ui, serial: bool) -> bool {
    let m = &ui.m;
    let Some(record) = ui.last[0].as_ref() else { return false };
    if !ui.src.is_hw()
        || board().slow_mode(m) != crate::board::Slow::Off
        || m.trig_mode == TrigMode::Single
        || m.segments != 0
        || (m.mask_on && m.mask_stop)
        || ui.hist_pos.is_some()
        || ui.seg_pos.is_some()
    {
        return false;
    }
    let frame = record.raw.as_ref().map(|raw| raw.frame.as_ref());
    serial
        || frame.is_some_and(|f| {
            crate::compress::live(f) || (crate::deepmeas::want(m).is_some() && crate::deepmeas::deep_pairs(f))
        })
        || m.trig_mode == TrigMode::Auto
            && (m.la_on() || frame.is_some_and(|f| f.acq.1 > crate::compress::MAX_DEPTH as usize))
}

pub(super) fn stage(ui: &mut Ui) {
    let Some(prepared) = ui.host.runtime_mut().change.prepared.as_mut() else { return };
    if std::mem::replace(&mut prepared.transitioned, true) {
        return;
    }
    let (running, run_edge, single) = (!prepared.stopped, prepared.run_edge, prepared.single);
    let id = prepared.changes.identity();
    let pair_changed = ui
        .host
        .runtime()
        .change
        .last
        .as_ref()
        .is_none_or(|c| (c.identity().gen(), c.identity().epoch()) != (id.gen(), id.epoch()));
    if running && ui.seg_hw_on() && (run_edge || single || pair_changed) {
        restart_segments(ui);
    } else if running && single && ui.m.segments > 0 {
        super::consumers::apply_classes(ui, C::HISTORY);
    }
}

pub(super) fn poll(ui: &mut Ui) {
    if ui.seg_hw_on() || ui.seg_view.is_some() || ui.seg_pos.is_some() {
        ui.seg_poll();
    }
    // stopped: a full-depth demo capture still under way is dropped
    if !ui.was_running {
        ui.cancel_demo_acquisition();
    }
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
