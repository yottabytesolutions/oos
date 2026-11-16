// SPDX-License-Identifier: GPL-2.0-or-later

//! Legacy acquisition and measurement actions, using the UI's shared setters and reset paths.
use super::*;

pub(super) fn meas_clear(scpi: &mut Scpi, ui: &mut Ui, arg: &str) {
    let a = arg.trim();
    if a.is_empty() || kw(a, "ALL") {
        ui.meas_clear_remote();
        return;
    }
    let (k, i) = suffix(a);
    if let Some(i) = i.filter(|&i| kw(k, "ITEM") && i >= 1 && i <= ui.measurement_count()) {
        ui.remove_measurement(i - 1);
        ui.mark_changed();
    } else {
        scpi.push_err(format!("-224,\"Illegal parameter value\" {}", a));
    }
}

pub(super) fn run(ui: &mut Ui, name: &str, query: bool) -> bool {
    if kw(name, "RUN") {
        if ui.m.run == RunState::Stop {
            crate::ui::change::run_request(ui);
        }
        ui.m.run = RunState::Run;
        if ui.m.trig_mode == TrigMode::Single {
            ui.m.trig_mode = TrigMode::Auto;
        }
    } else if kw(name, "STOP") {
        ui.m.run = RunState::Stop;
    } else if kw(name, "SINGle") {
        if !query {
            crate::ui::change::single(ui);
        }
        ui.m.trig_mode = TrigMode::Single;
        ui.m.run = RunState::Run;
    } else {
        return false;
    }
    ui.mark_changed();
    true
}
