// SPDX-License-Identifier: GPL-2.0-or-later

//! Acting pre-acquisition code extracted without changing its order. Only observation calls are new.

use super::super::*;
use super::oracle::Row;
use super::request;
use oos_core::change::Request;

pub(super) fn settings(ui: &mut Ui) {
    super::normalize::apply(ui);
    ui.src.publish_afg(&ui.m);
    // SINGLE armed: from here on only what comes after the press counts.
    let single = ui.m.trig_mode == TrigMode::Single && ui.m.run == RunState::Run;
    if single && !ui.single_armed {
        if !ui.host.runtime().change.pending.entries().contains(&oos_core::change::Command::Request(Request::Single)) {
            request(ui, Request::Single, Row::Single);
        }
        ui.m.hw_stamp += 1;
        ui.hw_avg_restart();
    }
    ui.single_armed = single;
    if ui.meas_src >= n_ch() || ui.ref_src >= n_ch() {
        (ui.meas_src, ui.ref_src) = (ui.meas_src.min(n_ch() - 1), ui.ref_src.min(n_ch() - 1));
    }
    super::prepare(ui);
    ui.auto_step();
    ui.sysinfo_poll();
    ui.cal_poll();
}

pub(super) fn transitions(ui: &mut Ui) -> bool {
    let running = ui.m.run == RunState::Run;
    let hw_seg = ui.seg_hw_on();
    if running && !ui.was_running {
        super::run_restart(ui);
    }
    if running && !ui.was_running && ui.m.segments > 0 {
        super::run_history(ui);
        ui.seg_n = 0;
        ui.history.clear();
        ui.hist_pos = None;
        if hw_seg {
            ui.seg_run += 1;
            (ui.seg_pos, ui.seg_view) = (None, None);
            ui.seg_times.clear();
        }
    }
    if !running && ui.was_running && ui.stop_wants_final() {
        ui.final_req += 1;
        ui.stop_final = Some(ui.now + STOP_FINAL_WAIT);
    }
    let final_done = ui.src.hw_hold(!running && ui.last[0].is_some(), ui.final_req);
    ui.src.hw_needs_raw(ui.needs_raw());
    ui.was_running = running;
    if hw_seg {
        ui.seg_poll(running);
    }
    final_done
}
