// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

/// Record the driver's exact position delta before the next intake can apply user edits.
pub(in crate::ui) fn playback_position(ui: &mut Ui, position: f32) {
    let zoom = ui.m.zoom;
    let before = (ui.m.delay, ui.m.zdelay);
    if zoom {
        ui.m.zdelay = position;
    } else {
        ui.m.delay = position;
    }
    ui.clamp_zoom();
    for (zoom, before, after) in [(false, before.0, ui.m.delay), (true, before.1, ui.m.zdelay)] {
        if before.to_bits() != after.to_bits() {
            request(
                ui,
                Request::PlaybackPosition { zoom, before: before.to_bits(), after: after.to_bits() },
                Row::Layout,
            );
        }
    }
}

/// Source installation is accepted before the driver presents captures during a step.
pub(in crate::ui) fn playback_setup(ui: &mut Ui) {
    stats_reset(ui);
    replay(ui);
}

/// Late SIM installation must reach acceptance before its first capture replaces live measurements.
/// Synchronous intake callers retain their immediate presentation; the next step accepts their setup.
pub(in crate::ui) fn playback_ready(ui: &Ui) -> bool {
    let observer = &ui.host.runtime().change;
    !observer.executing || !observer.pending.entries().iter().any(|c| matches!(c, Command::Request(Request::Replay)))
}

/// Retain the old analyzer reset observation until the table/list deletion hold is released.
pub(in crate::ui) fn analyzer_reset(ui: &mut Ui) {
    event(ui, Row::Analyzer, C::MATH | C::VIEW);
}
