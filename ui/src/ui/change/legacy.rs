// SPDX-License-Identifier: GPL-2.0-or-later

//! Remaining legacy consumers and the transitional settings intake.

use super::super::*;
use super::oracle::Row;
use super::request;
use oos_core::change::Request;

pub(super) fn settings(ui: &mut Ui) {
    if ui.host.runtime().change.prepared.is_none() {
        intake_settings(ui);
    }
    // the trigger level line shows for a while after a level change (`level_line_on`); the first look is no change
    if (ui.m.trig_level, ui.m.trig_level2) != ui.level_seen {
        if !ui.level_seen.0.is_nan() {
            ui.level_changed = ui.now;
        }
        ui.level_seen = (ui.m.trig_level, ui.m.trig_level2);
    }
    ui.sysinfo_poll();
    ui.cal_poll();
}

pub(super) fn intake_settings(ui: &mut Ui) {
    super::proposals::apply(ui);
    super::normalize::apply(ui);
    // SINGLE armed: from here on only what comes after the press counts.
    let single = ui.m.trig_mode == TrigMode::Single && ui.m.run == RunState::Run;
    if single
        && !ui.trigger.single_armed
        && !ui.host.runtime().change.pending.entries().contains(&oos_core::change::Command::Request(Request::Single))
    {
        request(ui, Request::Single, Row::Single);
    }
    ui.trigger.single_armed = single;
    ui.bus_candidate_guard_tick();
    for source in [&mut ui.meas_src, &mut ui.meas_src_b] {
        if matches!(*source, WaveId::Ch(c) if c >= n_ch()) {
            *source = WaveId::Ch(0);
        }
    }
    if ui.ref_src >= n_ch() {
        ui.ref_src = n_ch() - 1;
    }
    super::prepare(ui);
}
