// SPDX-License-Identifier: GPL-2.0-or-later

//! Invalidate picture inputs independently of the pixels still on screen.

use super::*;

pub(super) fn apply(ui: &mut Ui) -> bool {
    let Some(p) = &ui.host.runtime().change.prepared else { return false };
    let classes = p.changes.classes();
    let (clear, single, stopped) = (p.clear, p.single, p.stopped);
    let layout_changed = std::mem::take(&mut ui.batch_picture.layout_changed);
    let run = p.run_edge && ui.src.is_hw() && !ui.record_mode();
    let waiting = ui.batch_picture.replacement_pending;
    let frozen = !stopped && (waiting || (run && !single && has_picture(ui)));
    let erase = frozen && invalidates_frozen_picture(p, layout_changed);
    if run && ui.m.status == TrigStatus::Stop {
        ui.m.status = TrigStatus::Wait;
    }
    if p.run_edge {
        if let Some((capture, _)) = ui.stop_capture.take() {
            ui.last[0] = Some(capture);
        }
        ui.stop_pic = None;
        ui.stop_final = None;
    }
    if run || (waiting && single) {
        step_draw::retire_picture_inputs(ui);
    }
    ui.batch_picture.replacement_pending = frozen && !erase;
    if ui.batch_picture.replacement_pending {
        return false;
    }
    if erase {
        ui.last[0] = None;
        ui.cleared = true;
    }
    if erase
        || clear
        || ((classes.contains(C::MAPPING) || classes.contains(C::PHOSPHOR)) && (!stopped || ui.record_mode()))
    {
        step_draw::clear_maps(ui);
        return true;
    }
    false
}

fn invalidates_frozen_picture(change: &Prepared, layout_changed: bool) -> bool {
    change.clear
        || layout_changed
        || change.changes.classes().contains(C::MAPPING)
        || change
            .fields
            .iter()
            .flatten()
            .any(|field| matches!(field.name(), "acq" | "mdepth" | "deskew_ch") || field.name().ends_with(".unit"))
}

fn has_picture(ui: &Ui) -> bool {
    ui.last[0].as_ref().is_some_and(|record| record.la.is_some())
        || (0..NCH).any(|c| ui.ph.channel_max(c) != 0 || (ui.m.zoom && ui.phz.channel_max(c) != 0))
}
