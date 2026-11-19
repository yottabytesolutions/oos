// SPDX-License-Identifier: GPL-2.0-or-later

//! Accepted settings invalidate the maps once. STOP's ordinary redraw remains the
//! sole clear-and-draw operation there; Min replacement and decay remain drawing.

use super::*;

pub(super) fn apply(ui: &mut Ui) -> bool {
    let Some(p) = &ui.host.runtime().change.prepared else { return false };
    let classes = p.changes.classes();
    if (classes.contains(C::MAPPING) || classes.contains(C::PHOSPHOR)) && (!p.stopped || ui.record_mode()) {
        step_draw::reset_window(ui);
        ui.ph.clear();
        ui.phz.clear();
        return true;
    }
    false
}
