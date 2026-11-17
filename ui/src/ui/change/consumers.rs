// SPDX-License-Identifier: GPL-2.0-or-later

//! Reset effects run once for the accepted class union, before publication and records.

use super::*;

pub(super) fn apply(ui: &mut Ui) -> bool {
    let Some(p) = &mut ui.host.runtime_mut().change.prepared else { return false };
    if std::mem::replace(&mut p.consumed, true) {
        return false;
    }
    let c = p.changes.classes();
    let clear = p.clear;
    let sample = ui.host.runtime().change.census.begin();
    apply_classes(ui, c);
    if clear {
        clear_measurements(ui);
    }
    sample.finish(&mut ui.host.runtime_mut().change.census, 5);
    !c.without(C::PHOSPHOR).is_empty()
}

pub(in crate::ui) fn apply_classes(ui: &mut Ui, c: C) {
    if c.contains(C::STATS) {
        ui.clear_measurement_statistics();
        ui.meas_new = false;
    }
    if c.contains(C::MASK) {
        ui.mask_pass = 0;
        ui.mask_fail = 0;
        ui.mask_hw_clear += 1;
    }
    if c.contains(C::ZONE) {
        ui.zone_drops = 0;
        ui.zone_hw_clear += 1;
    }
    if c.contains(C::HW_AVERAGE) && ui.src.is_hw() {
        ui.m.hw_avg_clear += 1;
    }
    if c.contains(C::HISTORY) {
        ui.history.clear();
        ui.hist_pos = None;
        ui.seg_n = 0;
        ui.seg_times.clear();
        (ui.seg_pos, ui.seg_view) = (None, None);
    }
    super::views::apply(ui, c);
}

fn clear_measurements(ui: &mut Ui) {
    ui.stats = [Stats::default(); NCH];
    for analysis in &mut ui.ana {
        if let Some(value) = std::sync::Arc::get_mut(analysis) {
            *value = Default::default();
        } else {
            *analysis = Default::default();
        }
    }
    ui.meas_want = false;
    // the record waiting for the worker belongs to the cleared record (one rule, upstream 99dd12e9)
    acquisition::cancel_measurement(ui);
}
