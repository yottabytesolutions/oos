// SPDX-License-Identifier: GPL-2.0-or-later

//! Legacy acquisition actions, retaining the existing broad mark even for repeated values.
use super::*;

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
