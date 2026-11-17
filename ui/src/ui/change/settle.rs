// SPDX-License-Identifier: GPL-2.0-or-later

//! Acceptance of a prepared change: the step's path, and SCPI's, whose queries and message ends are change points.

use super::*;
use std::time::Duration;

/// Reset consumers, run transitions and publication for the prepared change. Returns whether a STOP final read is
/// done and the time the reset consumers took, which is not G2 change time.
pub(super) fn accept(ui: &mut Ui) -> (bool, Duration) {
    let consumer_start = now(ui);
    let reset = consumers::apply(ui);
    let consumer_time = if reset { since(ui, consumer_start) } else { Duration::ZERO };
    let final_done = transitions::apply(ui);
    if reset {
        ui.host.runtime_mut().change.timing.consumers(consumer_time);
    }
    acquisition::publish(ui);
    (final_done, consumer_time)
}

/// SCPI is sequential (IEEE 488.2): a query observes every command before it. Each query and the end of each program
/// message is therefore an accepted change point: detection, reset consumers, transitions, publication and the
/// picture, as in a step. The commands between two such points form one change, as edits within a step do. The step that
/// follows finds nothing prepared and runs its polls as usual. A change the script producer prepared stays for its
/// step, and a memory stop accepts nothing, as in the step. The SCPI intake that calls this stops its clock here: the
/// change, consumer and picture rings each take their own part, and no part enters two rings.
pub(crate) fn settle(ui: &mut Ui) {
    if ui.memory_stopped() || is_prepared(ui) {
        return;
    }
    intake::outside(ui, |ui| {
        let started = now(ui);
        legacy::intake_settings(ui);
        let (_, consumer_time) = accept(ui);
        let change = since(ui, started).saturating_sub(consumer_time);
        ui.host.runtime_mut().change.timing.queue_change(change);
        let started = now(ui);
        if picture::apply(ui) {
            let elapsed = since(ui, started);
            ui.host.runtime_mut().change.timing.picture(elapsed);
        }
        observe(ui);
        analyze_switched_view(ui);
    });
}

/// A page switch is built after the resets of the change that accepted it, in the step and at a SCPI change point
/// alike: the switch only records the view, the reset runs at acceptance, then the page is computed.
pub(in crate::ui) fn analyze_switched_view(ui: &mut Ui) {
    if ui.analyzed_view != ui.m.view {
        ui.analyzed_view = ui.m.view;
        if ui.m.view != AView::Scope {
            ui.analyzer_update();
        }
    }
}
